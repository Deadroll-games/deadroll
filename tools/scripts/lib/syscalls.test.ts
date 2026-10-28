import { test } from "node:test";
import assert from "node:assert/strict";
import { GATED_SYSCALLS, featureActive, gatedSyscallsIn, syscallHash, syscallProblems } from "./syscalls.ts";

/** One 8-byte SBPF instruction: opcode, registers, offset 0, immediate. */
function insn(opcode: number, imm: number): Buffer {
  const b = Buffer.alloc(8);
  b[0] = opcode;
  b.writeUInt32LE(imm, 4);
  return b;
}

const [remaining, blake3] = GATED_SYSCALLS;

test("syscall ids match the loader's murmur3 of the name", () => {
  assert.equal(syscallHash("abort"), 0xb6fc1a11);
  assert.equal(syscallHash("sol_panic_"), 0x686093bb);
  assert.equal(syscallHash("sol_log_"), 0x207559bd);
});

test("an SBPF v3 syscall or a v0 call by id is found, aligned only", () => {
  const nop = insn(0xb7, 0);
  assert.deepEqual(gatedSyscallsIn(Buffer.concat([nop, insn(0x95, syscallHash(remaining.name)), nop])), [remaining]);
  assert.deepEqual(gatedSyscallsIn(Buffer.concat([insn(0x85, syscallHash(blake3.name))])), [blake3]);
  // The same bytes 4 bytes off the instruction grid are data, not a call.
  assert.deepEqual(gatedSyscallsIn(Buffer.concat([Buffer.alloc(4), insn(0x95, syscallHash(remaining.name)), Buffer.alloc(4)])), []);
  assert.deepEqual(gatedSyscallsIn(Buffer.concat([nop, insn(0x95, syscallHash("sol_log_"))])), []);
});

test("a syscall named in the ELF string table is found", () => {
  assert.deepEqual(gatedSyscallsIn(Buffer.from(`\0abort\0${blake3.name}\0`, "latin1")), [blake3]);
  assert.deepEqual(gatedSyscallsIn(Buffer.from(`\0${blake3.name}_extra\0`, "latin1")), []);
});

test("a feature is active only with Some(slot)", () => {
  assert.equal(featureActive(null), false);
  assert.equal(featureActive(new Uint8Array([0])), false);
  assert.equal(featureActive(new Uint8Array([1, 5, 0, 0, 0, 0, 0, 0, 0])), true);
});

test("only syscalls inactive on the cluster are problems", () => {
  const programs = [
    { name: "a", so: insn(0x95, syscallHash(remaining.name)) },
    { name: "b", so: insn(0x95, syscallHash("sol_log_")) },
  ];
  const problems = syscallProblems(programs, GATED_SYSCALLS);
  assert.equal(problems.length, 1);
  assert.match(problems[0], /^a\.so calls sol_remaining_compute_units \(feature 5Tupp/);
  assert.deepEqual(syscallProblems(programs, [blake3]), []);
});
