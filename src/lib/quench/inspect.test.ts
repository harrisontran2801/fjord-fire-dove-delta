import assert from "node:assert/strict";
import { gzipSync } from "node:zlib";
import { test } from "node:test";
import {
  UNSUPPORTED_FORMAT,
  artifactFromFile,
  inspectFile,
} from "./inspect.ts";

function fileFrom(bytes: Uint8Array, name: string): File {
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  return new File([copy.buffer], name);
}

function encodeUtf8(text: string): Uint8Array {
  return new TextEncoder().encode(text);
}

function tarHeader(name: string, size: number, type = "0"): Uint8Array {
  const header = new Uint8Array(512);
  const write = (offset: number, value: string, max: number) => {
    const bytes = encodeUtf8(value).slice(0, max);
    header.set(bytes, offset);
  };
  write(0, name, 99);
  write(100, "0000644", 7);
  write(108, "0000000", 7);
  write(116, "0000000", 7);
  write(124, size.toString(8).padStart(11, "0"), 11);
  write(136, "00000000000", 11);
  header.fill(0x20, 148, 156);
  header[156] = type.charCodeAt(0);
  write(257, "ustar", 5);
  write(263, "00", 2);
  let sum = 0;
  for (const b of header) sum += b;
  write(148, `${sum.toString(8).padStart(6, "0")}\0 `, 8);
  return header;
}

function buildTar(files: { name: string; content: string | Uint8Array }[]): Uint8Array {
  const chunks: Uint8Array[] = [];
  for (const file of files) {
    const content = typeof file.content === "string" ? encodeUtf8(file.content) : file.content;
    chunks.push(tarHeader(file.name, content.length));
    chunks.push(content);
    const pad = (512 - (content.length % 512)) % 512;
    if (pad) chunks.push(new Uint8Array(pad));
  }
  chunks.push(new Uint8Array(1024));
  const total = chunks.reduce((n, c) => n + c.length, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return out;
}

function elfBytes(payload: number, extra = 192): Uint8Array {
  const bytes = new Uint8Array(64 + extra);
  bytes.set([0x7f, 0x45, 0x4c, 0x46, 2, 1, 1], 0);
  bytes[18] = 62;
  bytes[19] = 0;
  bytes[40] = payload;
  return bytes;
}

function dockerTar(tag = "demo/app:1.0"): Uint8Array {
  return buildTar([
    {
      name: "manifest.json",
      content: JSON.stringify([
        { Config: "config.json", RepoTags: [tag], Layers: ["layer/layer.tar"] },
      ]),
    },
    { name: "config.json", content: JSON.stringify({ architecture: "amd64", os: "linux" }) },
    { name: "layer/layer.tar", content: new Uint8Array([1, 2, 3, 4]) },
  ]);
}

function ociTar(): Uint8Array {
  return buildTar([
    { name: "oci-layout", content: JSON.stringify({ imageLayoutVersion: "1.0.0" }) },
    {
      name: "index.json",
      content: JSON.stringify({
        schemaVersion: 2,
        manifests: [{ mediaType: "application/vnd.oci.image.manifest.v1+json", digest: "sha256:abc" }],
      }),
    },
    { name: "blobs/sha256/abc", content: new Uint8Array([9, 8, 7]) },
  ]);
}

test("rejects MSI by OLE magic, not by size", async () => {
  const bytes = new Uint8Array(4096);
  bytes.set([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]);
  const file = fileFrom(bytes, "setup.msi");
  const result = await artifactFromFile(file);
  assert.equal(result.ok, false);
  if (!result.ok) assert.equal(result.error, UNSUPPORTED_FORMAT);
});

test("rejects PE/EXE by MZ magic", async () => {
  const bytes = new Uint8Array(512);
  bytes[0] = 0x4d;
  bytes[1] = 0x5a;
  const result = await inspectFile(fileFrom(bytes, "app.exe"));
  assert.equal(result.ok, false);
  if (!result.ok) assert.equal(result.error, UNSUPPORTED_FORMAT);
});

test("does not classify a large random file as Docker", async () => {
  const bytes = new Uint8Array(120_000);
  bytes.fill(0x41);
  const file = fileFrom(bytes, "docker-image.container.tar");
  const result = await inspectFile(file);
  assert.equal(result.ok, false);
});

test("identifies a real ELF and hashes file contents", async () => {
  const bytes = elfBytes(7);
  const file = fileFrom(bytes, "match-engine");
  const result = await artifactFromFile(file);
  assert.equal(result.ok, true);
  if (!result.ok) return;
  assert.equal(result.artifact.kind, "elf");
  assert.equal(result.artifact.source, "upload");
  assert.equal(result.artifact.language.includes("x86-64"), true);
  assert.equal(result.artifact.sha256?.length, 64);
  assert.equal(result.artifact.sampleId, undefined);
});

test("identifies a Docker save archive from tar + manifest.json", async () => {
  const file = fileFrom(dockerTar("ghcr.io/acme/api:1"), "api.tar");
  const result = await artifactFromFile(file);
  assert.equal(result.ok, true);
  if (!result.ok) return;
  assert.equal(result.artifact.kind, "docker");
  assert.equal(result.artifact.archive?.format, "docker-save");
  assert.deepEqual(result.artifact.archive?.repoTags, ["ghcr.io/acme/api:1"]);
});

test("identifies a gzipped Docker save", async () => {
  const gz = gzipSync(Buffer.from(dockerTar("demo:latest")));
  const file = fileFrom(new Uint8Array(gz), "demo.tar.gz");
  const result = await inspectFile(file);
  assert.equal(result.ok, true);
  if (!result.ok) return;
  assert.equal(result.kind, "docker");
});

test("identifies an OCI archive from oci-layout", async () => {
  const file = fileFrom(ociTar(), "image.oci.tar");
  const result = await artifactFromFile(file);
  assert.equal(result.ok, true);
  if (!result.ok) return;
  assert.equal(result.artifact.kind, "oci");
  assert.equal(result.artifact.archive?.format, "oci");
});

test("same name and size but different contents get different SHA-256", async () => {
  const a = elfBytes(1);
  const b = elfBytes(2);
  assert.equal(a.byteLength, b.byteLength);
  const fa = fileFrom(a, "worker");
  const fb = fileFrom(b, "worker");
  const ra = await inspectFile(fa);
  const rb = await inspectFile(fb);
  assert.equal(ra.ok, true);
  assert.equal(rb.ok, true);
  if (!ra.ok || !rb.ok) return;
  assert.notEqual(ra.sha256, rb.sha256);
  assert.equal(ra.sizeBytes, rb.sizeBytes);
  assert.equal(ra.name, rb.name);
});

test("a tar without Docker/OCI structure is unsupported", async () => {
  const tar = buildTar([{ name: "readme.txt", content: "hello" }]);
  const result = await inspectFile(fileFrom(tar, "notes.tar"));
  assert.equal(result.ok, false);
  if (!result.ok) assert.equal(result.error, UNSUPPORTED_FORMAT);
});
