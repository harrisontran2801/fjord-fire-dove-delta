//! Deterministic bytecode interpreter used as a second Quench technical sample.
//! The dispatch is a function-pointer table. That is ordinary interpreter shape,
//! not a layout tuned to force a BOLT win.

const TAPE: usize = 65_536;
const MEM: usize = 8_192;
const OPS: usize = 16;

pub struct Vm {
    mem: Vec<u64>,
    code: Vec<u8>,
    acc: u64,
    reg: u64,
    ip: usize,
    flag: bool,
}

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    if x == 0 {
        x = 0x9e37_79b9_7f4a_7c15;
    }
    *state = x;
    x
}

fn tape(seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    let mut code = vec![0u8; TAPE];
    for b in &mut code {
        *b = (xorshift(&mut state) & 0xff) as u8;
    }
    code
}

fn op_add(vm: &mut Vm) {
    let i = (vm.ip ^ vm.acc as usize) % MEM;
    vm.acc = vm.acc.wrapping_add(vm.mem[i]);
}

fn op_xor(vm: &mut Vm) {
    let i = vm.ip % MEM;
    vm.acc ^= vm.mem[i].rotate_left((vm.code[vm.ip % TAPE] & 31) as u32);
}

fn op_mul(vm: &mut Vm) {
    vm.acc = vm.acc.wrapping_mul(3).wrapping_add(vm.reg | 1);
}

fn op_shift(vm: &mut Vm) {
    let n = (vm.code[(vm.ip + 1) % TAPE] & 63) as u32;
    vm.acc = vm.acc.rotate_left(n) ^ vm.acc.rotate_right(n.saturating_sub(1));
}

fn op_store(vm: &mut Vm) {
    let i = (vm.acc as usize ^ vm.ip) % MEM;
    vm.mem[i] = vm.acc ^ vm.reg;
}

fn op_load(vm: &mut Vm) {
    let i = (vm.reg as usize).wrapping_add(vm.ip) % MEM;
    vm.acc = vm.mem[i].wrapping_add(vm.acc >> 3);
}

fn op_branch_even(vm: &mut Vm) {
    vm.flag = vm.acc & 1 == 0;
    if vm.flag {
        vm.ip = vm.ip.wrapping_add((vm.code[vm.ip % TAPE] as usize) & 63);
    }
}

fn op_branch_odd(vm: &mut Vm) {
    if vm.acc & 1 == 1 {
        let back = (vm.code[(vm.ip + 3) % TAPE] as usize) & 31;
        vm.ip = vm.ip.wrapping_sub(back);
        vm.flag = true;
    } else {
        vm.flag = false;
    }
}

fn op_swap(vm: &mut Vm) {
    std::mem::swap(&mut vm.acc, &mut vm.reg);
}

fn op_not(vm: &mut Vm) {
    vm.acc = !vm.acc;
    vm.flag = vm.acc.count_ones() & 1 == 0;
}

fn op_imm(vm: &mut Vm) {
    let b = vm.code[(vm.ip.wrapping_mul(3) + 7) % TAPE] as u64;
    vm.acc = vm.acc.wrapping_add(b.wrapping_mul(0x1000_0000_1b3));
}

fn op_cmp(vm: &mut Vm) {
    let i = (vm.ip.wrapping_add(vm.reg as usize)) % MEM;
    vm.flag = vm.acc < vm.mem[i];
    if vm.flag {
        vm.acc = vm.acc.wrapping_add(1);
    } else {
        vm.reg = vm.reg.wrapping_add(vm.mem[i] & 0xff);
    }
}

fn op_lookup(vm: &mut Vm) {
    let i = (vm.acc as usize).wrapping_mul(17).wrapping_add(vm.ip) % MEM;
    let j = (i.wrapping_mul(3) + 11) % MEM;
    vm.reg = vm.mem[i] ^ vm.mem[j];
    vm.acc = vm.acc.wrapping_add(vm.reg >> 5);
}

fn op_push_ip(vm: &mut Vm) {
    let i = vm.ip % MEM;
    vm.mem[i] = vm.mem[i].wrapping_add(vm.ip as u64);
    vm.reg ^= vm.ip as u64;
}

fn op_pop_mix(vm: &mut Vm) {
    let i = (vm.reg as usize) % MEM;
    vm.acc = vm.acc.wrapping_add(vm.mem[i]);
    vm.mem[i] = vm.acc.rotate_left(7);
}

fn op_mix(vm: &mut Vm) {
    let i = (vm.ip.wrapping_add(13)) % MEM;
    vm.mem[i] ^= vm.acc.wrapping_add(vm.reg);
    if vm.flag {
        vm.acc = vm.acc.wrapping_sub(vm.mem[i] & 0xffff);
    } else {
        vm.reg = vm.reg.wrapping_add(vm.mem[i] >> 8);
    }
}

const DISPATCH: [fn(&mut Vm); OPS] = [
    op_add,
    op_xor,
    op_mul,
    op_shift,
    op_store,
    op_load,
    op_branch_even,
    op_branch_odd,
    op_swap,
    op_not,
    op_imm,
    op_cmp,
    op_lookup,
    op_push_ip,
    op_pop_mix,
    op_mix,
];

pub fn execute(steps: u32, seed: u64) -> u64 {
    let mut vm = Vm {
        mem: vec![seed; MEM],
        code: tape(seed),
        acc: seed,
        reg: seed ^ 0x5a5a_5a5a_5a5a_5a5a,
        ip: (seed as usize) % TAPE,
        flag: false,
    };
    let mut checksum = seed;
    for step in 0..steps {
        let op = vm.code[vm.ip % TAPE];
        DISPATCH[(op as usize) % OPS](&mut vm);
        checksum = checksum.wrapping_add(vm.acc ^ (step as u64));
        vm.ip = vm.ip.wrapping_add(1 + ((vm.acc as usize) & 7));
    }
    checksum ^ vm.reg ^ vm.acc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_is_deterministic() {
        assert_eq!(execute(20_000, 42), execute(20_000, 42));
    }

    #[test]
    fn different_seed_changes_checksum() {
        assert_ne!(execute(20_000, 42), execute(20_000, 99));
    }

    #[test]
    fn golden_checksum_catches_a_wrong_transform() {
        assert_eq!(execute(8_192, 7), 4_168_958_530_509_816_306);
    }
}
