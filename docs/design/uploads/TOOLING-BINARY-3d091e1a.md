# Binmap — Binary Toolchain Reference

> Working reference for `object`, `gimli`, `addr2line`, `rustc-demangle`, and `iced-x86`.
> Companion to `TOOLING.md`, which covers *why*; this covers *how*.
>
> **API signatures shift between versions of these crates.** Everything below reflects the
> shape of the APIs, not a guarantee for the version you pin. Check against your locked
> version before committing to a design — especially `gimli` and `addr2line`, which have both
> restructured their reader traits.

---

## 1. `object` — container parsing

### 1.1 The high-level path

```rust
use object::{Object, ObjectSection, ObjectSymbol, ObjectSegment, File, SymbolKind};

let data = std::fs::read(path)?;          // keep alive; File borrows from it
let file = File::parse(&*data)?;

file.format();          // BinaryFormat::Elf
file.architecture();    // Architecture::X86_64
file.is_64();
file.endianness();
file.entry();
file.relative_address_base();
file.build_id()?;       // Option<&[u8]>  — how you match binary ↔ debug info ↔ core
file.gnu_debuglink()?;  // Option<(&[u8], u32)> — filename + CRC of separate debug file
file.has_debug_symbols();
```

**Lifetime shape matters.** `File<'data>` borrows the mmap or `Vec`. Everything downstream
(`gimli`, `addr2line`) borrows further. In a GUI where an analysis outlives a function scope,
the practical pattern is to own the bytes in an `Arc<Mmap>` and build a self-referential
holder, or to use `owning_ref`/`yoke` style ownership. Decide this early: retrofitting
lifetimes through an analysis engine is miserable.

```rust
// The pattern that works: own the map, hand out borrows from a stable address.
pub struct LoadedBinary {
    _map: Arc<memmap2::Mmap>,   // never moved
    file: object::File<'static>, // transmuted lifetime, safe because _map is Arc'd and pinned
}
```

Yes, that transmute needs a comment and a test. The alternative — re-parsing on every query —
is worse.

### 1.2 Sections

```rust
for section in file.sections() {
    section.index();
    section.name()?;                    // ".text"
    section.address();                  // link-time vaddr
    section.size();                     // in-memory size
    section.align();
    section.file_range();               // Option<(offset, size)> — None for .bss
    section.kind();                     // SectionKind::Text | ReadOnlyData | ...
    section.flags();                    // SectionFlags::Elf { sh_flags }
    section.data()?;                    // raw bytes
    section.uncompressed_data()?;       // Cow — handles SHF_COMPRESSED / .zdebug_
    section.compressed_file_range()?;
}
```

**`uncompressed_data()` is not optional.** Compressed debug sections are the default on many
distributions. Reading `.debug_info` with `data()` on a compressed section gives you zlib
bytes and a confusing parse failure.

**`.bss` has size but no file range.** Any code that sums `file_range().1` will undercount
memory footprint and overcount nothing. Decide per feature whether you mean file size or VM
size and label it; `bloaty`'s `--domain` flag exists for exactly this reason.

### 1.3 Symbols

```rust
for sym in file.symbols() {          // .symtab
    sym.index();
    sym.name()?;                     // still mangled
    sym.address();
    sym.size();
    sym.kind();                      // SymbolKind::Text | Data | Section | File
    sym.section();                   // SymbolSection::Section(idx) | Undefined | Absolute
    sym.scope();                     // Compilation | Linkage | Dynamic
    sym.is_global(); sym.is_weak(); sym.is_undefined();
}
file.dynamic_symbols();              // .dynsym — what survives stripping
file.symbol_by_index(idx)?;
```

**The size problem, in detail.** `st_size` is frequently 0 for assembly-defined symbols,
compiler-generated thunks, and some linker-produced symbols. It can also overlap with the next
symbol, or extend past the section end. Naive "size = next_symbol.address - this.address"
breaks on:

- Multiple symbols at the same address (aliases, `weak` + `global` pairs)
- Section boundaries — the next symbol may be in a different section
- Alignment padding between functions, which belongs to nobody
- `SHN_ABS` symbols, which have no meaningful address ordering

The algorithm that actually works:

```rust
// 1. Collect only symbols with a real section and Text/Data kind.
// 2. Group by section index.
// 3. Within a section, sort by (address, -st_size) so the largest alias sorts first.
// 4. Dedupe by address, keeping the first.
// 5. size = st_size if nonzero, else min(next.address, section.end) - this.address.
// 6. Track the gap between computed-end and next-start as "padding", attributed to
//    the section, not to any symbol.
```

Step 6 is what lets your totals reconcile with `bloaty`. Without it you will have a few
percent of unexplained bytes and no idea where they went.

### 1.4 Core dumps — the low-level path

No crate does this. `object`'s raw ELF module is the substrate.

```rust
use object::read::elf::{ElfFile64, FileHeader, ProgramHeader};
use object::{elf, Endianness};

let elf = ElfFile64::<Endianness>::parse(&*data)?;
let endian = elf.endian();
let header = elf.raw_header();
assert_eq!(header.e_type(endian), elf::ET_CORE);

for ph in elf.raw_segments() {
    match ph.p_type(endian) {
        elf::PT_LOAD => { /* memory contents at p_vaddr, p_filesz bytes at p_offset */ }
        elf::PT_NOTE => {
            if let Some(mut notes) = ph.notes(endian, &*data)? {
                while let Some(note) = notes.next()? {
                    match (note.name(), note.n_type(endian)) {
                        (b"CORE", elf::NT_PRSTATUS) => parse_prstatus(note.desc()),
                        (b"CORE", elf::NT_PRPSINFO) => parse_prpsinfo(note.desc()),
                        (b"CORE", elf::NT_AUXV)     => parse_auxv(note.desc()),
                        (b"CORE", NT_FILE)          => parse_nt_file(note.desc()),
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }
}
```

`NT_FILE` is `0x46494c45` (ASCII `"FILE"`) and is not always in `object`'s constants.

**`NT_PRSTATUS` layout on x86-64.** The registers sit inside `struct elf_prstatus` at a fixed
offset. Derivation:

| Field | Size | Cumulative |
|---|---|---|
| `pr_info` (`elf_siginfo`) | 12 | 12 |
| `pr_cursig` (`i16`) + pad | 4 | 16 |
| `pr_sigpend` (`u64`) | 8 | 24 |
| `pr_sighold` (`u64`) | 8 | 32 |
| `pr_pid`, `pr_ppid`, `pr_pgrp`, `pr_sid` (4 × `i32`) | 16 | 48 |
| `pr_utime`, `pr_stime`, `pr_cutime`, `pr_cstime` (4 × `timeval`) | 64 | 112 |
| **`pr_reg`** (`user_regs_struct`, 27 × `u64`) | 216 | 328 |

So registers start at **offset 112**, and there is one `NT_PRSTATUS` note per thread, in
order, with the crashing thread first.

**`user_regs_struct` field order** (this is `ptrace` order, and it is *not* DWARF order):

```
 0 r15   1 r14   2 r13   3 r12   4 rbp   5 rbx   6 r11   7 r10
 8 r9    9 r8   10 rax  11 rcx  12 rdx  13 rsi  14 rdi  15 orig_rax
16 rip  17 cs   18 eflags 19 rsp 20 ss  21 fs_base 22 gs_base
23 ds   24 es   25 fs   26 gs
```

**The trap that will cost you a day: DWARF numbers these registers differently.**

```
DWARF x86-64:  0 rax  1 rdx  2 rcx  3 rbx  4 rsi  5 rdi  6 rbp  7 rsp
               8-15 r8-r15   16 return address (rip)
```

`gimli::X86_64::RAX` is `0`; in `user_regs_struct`, index `0` is `r15`. A location list saying
"the variable is in register 6" means `rbp` in DWARF and `r11` in ptrace order. Write one
conversion table, test it against a known core, and never index a register array directly.

```rust
/// DWARF register number -> index into user_regs_struct.
const DWARF_TO_PRSTATUS: [usize; 17] = [
    10, // 0  rax
    12, // 1  rdx
    11, // 2  rcx
     5, // 3  rbx
    13, // 4  rsi
    14, // 5  rdi
     4, // 6  rbp
    19, // 7  rsp
     9, // 8  r8
     8, // 9  r9
     7, // 10 r10
     6, // 11 r11
     3, // 12 r12
     2, // 13 r13
     1, // 14 r14
     0, // 15 r15
    16, // 16 return address == rip
];
```

**`NT_FILE` format:** `count: u64`, `page_size: u64`, then `count` triples of
`(start: u64, end: u64, file_offset: u64)`, then `count` NUL-terminated path strings in the
same order. This is how you find the load bias.

**The PIE load bias.** Rust binaries are PIE by default. Every address in the core is runtime;
every address in DWARF is link-time.

```rust
// bias = runtime_start_of_main_executable_text - link_time_vaddr_of_same
let bias = nt_file_entry_for_exe.start - exe_first_pt_load_vaddr;
let dwarf_addr = runtime_addr - bias;
```

`AT_PHDR` from `NT_AUXV` gives an alternative derivation and is a good cross-check. **Assert
these two agree.** If they disagree, refuse to symbolize rather than producing confident
nonsense — this is exactly the "confident-wrong" failure the PRD's trust metric targets.

---

## 2. `gimli` — DWARF

### 2.1 Loading

```rust
use gimli::{Dwarf, EndianSlice, RunTimeEndian, SectionId};
use std::borrow::Cow;

let endian = if file.is_little_endian() { RunTimeEndian::Little } else { RunTimeEndian::Big };

let load_section = |id: SectionId| -> Result<Cow<[u8]>, gimli::Error> {
    Ok(match file.section_by_name(id.name()) {
        Some(s) => s.uncompressed_data().unwrap_or(Cow::Borrowed(&[])),
        None => Cow::Borrowed(&[]),
    })
};

let dwarf_cow = Dwarf::load(&load_section)?;
let dwarf = dwarf_cow.borrow(|s| EndianSlice::new(s, endian));
```

`SectionId::name()` gives the ELF name; there is also `.dwo_name()` for split units. Missing
sections must return empty slices rather than erroring — plenty of binaries lack
`.debug_loclists` entirely.

**Separate debug files.** Check `gnu_debuglink` and the `.build-id` path
(`/usr/lib/debug/.build-id/ab/cdef....debug`). Load DWARF from the debug file while loading
symbols and code bytes from the stripped original — they are two different `object::File`s
describing one program.

### 2.2 Walking units and DIEs

```rust
let mut iter = dwarf.units();
while let Some(header) = iter.next()? {
    let unit = dwarf.unit(header)?;
    let mut entries = unit.entries();
    while let Some((_delta_depth, entry)) = entries.next_dfs()? {
        if entry.tag() == gimli::DW_TAG_subprogram {
            let name = entry.attr(gimli::DW_AT_name)?
                .and_then(|a| dwarf.attr_string(&unit, a.value()).ok());
            let low  = entry.attr_value(gimli::DW_AT_low_pc)?;
            let high = entry.attr_value(gimli::DW_AT_high_pc)?;
        }
    }
}
```

`next_dfs()` yields depth deltas — you must track depth yourself to know the tree shape.
`next_sibling()` skips a subtree, which matters for performance when scanning for one tag.

**Attribute value forms you must handle:**

- `DW_AT_high_pc` is either an **address** (`AttributeValue::Addr`) or an **offset from
  low_pc** (`AttributeValue::Udata`), depending on DWARF version and producer. Handling only
  one produces function ranges that are wildly wrong. This is a classic.
- Strings arrive as `DebugStrRef`, `DebugLineStrRef`, `StringOffsetsIndex`, or inline
  `String`. Always resolve through `dwarf.attr_string()`.
- Addresses may be `DebugAddrIndex` (split DWARF), resolved via `dwarf.address()`.
- Cross-unit references (`DW_FORM_ref_addr`) require resolving against the right unit, not the
  current one. Type references frequently cross units under LTO.

**`DW_AT_ranges`** rather than `low_pc`/`high_pc` is common for functions after optimization —
a function's code can be split into non-contiguous ranges. Use `dwarf.die_ranges(&unit, entry)`
and handle both forms.

### 2.3 The line program

```rust
if let Some(program) = unit.line_program.clone() {
    let mut rows = program.rows();
    while let Some((header, row)) = rows.next_row()? {
        if row.end_sequence() { continue; }   // sentinel, not a real row
        let addr = row.address();
        let line = row.line();                // Option<NonZeroU64>
        let col  = row.column();
        let is_stmt = row.is_stmt();
        let file = row.file(header);          // resolve via header.file_names()
    }
}
```

Facts that shape the UI:

- **Rows are sorted by address within a sequence, but there may be many sequences**, and
  sequences are not globally ordered. Sort and merge before building an index.
- **One line maps to many disjoint address ranges** after optimization. This is the
  observation the source↔disassembly sync view is built on, not an anomaly.
- `end_sequence` rows carry the end address of the preceding sequence and no valid line.
- `line` can be `0`, meaning "no source line" — compiler-generated code.
- File index base differs between DWARF 4 (1-based, with index 0 meaning the primary file) and
  DWARF 5 (0-based). Get this wrong and every filename is off by one.

Build a sorted `Vec<(addr, LineEntry)>` once and binary-search it. Do not re-run the line
program per lookup.

### 2.4 Location lists — recovering "optimized out"

This is the hardest and highest-value part of Phase 2.

```rust
use gimli::{AttributeValue, EvaluationResult, Location, Piece, Value};

let AttributeValue::LocationListsRef(off) = entry.attr_value(gimli::DW_AT_location)?.unwrap()
    else { /* may also be Exprloc for a constant location */ };

let mut locs = dwarf.locations(&unit, off)?;
while let Some(entry) = locs.next()? {
    // entry.range: gimli::Range { begin, end } — link-time addresses
    if !(entry.range.begin..entry.range.end).contains(&pc) { continue; }

    let mut eval = entry.data.evaluation(unit.encoding());
    let mut result = eval.evaluate()?;
    loop {
        result = match result {
            EvaluationResult::Complete => break,

            EvaluationResult::RequiresRegister { register, base_type } => {
                let raw = read_reg(register)?;              // via DWARF_TO_PRSTATUS
                eval.resume_with_register(Value::Generic(raw))?
            }
            EvaluationResult::RequiresFrameBase => {
                eval.resume_with_frame_base(cfa)?           // from the unwinder
            }
            EvaluationResult::RequiresMemory { address, size, space: _, base_type: _ } => {
                let bytes = read_core_memory(address, size)?;
                eval.resume_with_memory(Value::Generic(u64_from_le(&bytes)))?
            }
            EvaluationResult::RequiresRelocatedAddress { .. }
            | EvaluationResult::RequiresTls { .. }
            | EvaluationResult::RequiresEntryValue { .. }
            | EvaluationResult::RequiresCallFrameCfa
            | EvaluationResult::RequiresBaseType { .. }
            | EvaluationResult::RequiresIndexedAddress { .. }
            | EvaluationResult::RequiresAtLocation { .. } => {
                return Err(Unsupported(result));   // handle explicitly, never guess
            }
        };
    }

    for piece in eval.result() {
        match piece.location {
            Location::Register { register }  => { /* value in a register */ }
            Location::Address { address }    => { /* value in memory */ }
            Location::Value { value }        => { /* the value itself, no storage */ }
            Location::Bytes { value }        => { /* literal bytes */ }
            Location::Empty                  => { /* THIS piece is optimized out */ }
            Location::ImplicitPointer { .. } => { /* pointer with no storage */ }
        }
        piece.size_in_bits;   // Option<u64> — Some means partial
        piece.bit_offset;
    }
}
```

**Five things that make this hard:**

1. **Piecewise locations are normal at `-O2`.** A 16-byte struct might be half in `rax` and
   half at `CFA-8`. `eval.result()` returns a `Vec<Piece>` and you must reassemble, respecting
   `size_in_bits` and `bit_offset`, in the target's endianness. A single-piece assumption
   silently produces wrong values.

2. **`Location::Empty` means that piece genuinely does not exist.** Report it as absent. Do
   not fill it with zeros. A partially-recovered struct must be rendered as partial in the UI.

3. **`RequiresFrameBase` needs the CFA**, which means the unwinder must run before variable
   evaluation. Order your Phase 2 pipeline: unwind first, then evaluate.

4. **`RequiresEntryValue` (`DW_OP_entry_value`) is increasingly common** and is genuinely
   hard — it means "the value this had at function entry," which requires evaluating an
   expression in the *caller's* frame. Supporting it is a real feature; the right v1 move is
   to detect and report "value requires entry-value evaluation, not supported" rather than
   fail opaquely.

5. **Location list address bases.** DWARF 5 `.debug_loclists` entries can be base-relative
   (`DW_LLE_offset_pair`) requiring `DW_AT_low_pc` of the unit as a base. `gimli` handles most
   of this if you construct through `dwarf.locations()` rather than parsing raw.

**A single-piece register value is not the variable.** You still need the type from
`DW_AT_type` to interpret the bits — a `u32` in `rax` uses the low 32 bits, an `f64` needs
reinterpretation, an enum needs discriminant decoding. Walk the type DIE tree
(`DW_TAG_base_type`, `DW_TAG_structure_type` with `DW_AT_data_member_location`,
`DW_TAG_variant_part` for Rust enums) and build a layout model. Rust enums in DWARF use
`DW_TAG_variant_part` with `DW_AT_discr`, and niche-optimized enums encode the discriminant
inside a field — this is where `Option<&T>` being a bare pointer becomes visible.

### 2.5 Unwinding with CFI

```rust
use gimli::{EhFrame, BaseAddresses, UnwindContext, UnwindSection, CfaRule, RegisterRule, X86_64};

let eh_frame = EhFrame::new(eh_frame_bytes, endian);
let bases = BaseAddresses::default()
    .set_eh_frame(eh_frame_section_addr)
    .set_eh_frame_hdr(hdr_addr)
    .set_text(text_addr)
    .set_got(got_addr);

let mut ctx = UnwindContext::new();
let row = eh_frame.unwind_info_for_address(&bases, &mut ctx, pc, EhFrame::cie_from_offset)?;

let cfa = match row.cfa() {
    CfaRule::RegisterAndOffset { register, offset } => read_reg(*register)? .wrapping_add(*offset as u64),
    CfaRule::Expression(expr) => evaluate(expr)?,
};
let return_addr = match row.register(X86_64::RA) {
    RegisterRule::Offset(o) => read_mem_u64(cfa.wrapping_add(o as u64))?,
    RegisterRule::Register(r) => read_reg(r)?,
    RegisterRule::Undefined => return Ok(StackEnd),   // outermost frame
    other => return Err(Unsupported(other)),
};
```

**Recommendation stands: use `framehop` instead** unless you need something it cannot express.
The cases it handles that hand-rolled CFI misses — missing or wrong `.eh_frame`, frame-pointer
fallback, prologue analysis, signal frames — are exactly the cases that appear in real crashes
rather than in test binaries.

Set `bases` correctly. Wrong base addresses produce FDE lookups that succeed and return the
wrong function, which unwinds into fiction.

### 2.6 Split DWARF

`-Csplit-debuginfo=unpacked` produces `.dwo` files; `packed` produces a `.dwp`. The skeleton
unit in the main binary carries `DW_AT_dwo_name` (or `DW_AT_GNU_dwo_name`) and
`DW_AT_dwo_id` / `DW_AT_GNU_dwo_id`. Full DIE data lives in the companion.

`gimli` supports this through `Dwarf::load` on the `.dwo` sections plus
`DwarfPackage` for `.dwp`. Detect the skeleton case and either load the companion or report
clearly that debug info is incomplete. Silently treating a skeleton unit as a full unit
produces a binary that appears to have almost no debug info.

---

## 3. `addr2line`

### 3.1 The core call

```rust
use addr2line::Context;

let ctx = Context::new(&object_file)?;
// or, reusing the gimli Dwarf you already built:
let ctx = Context::from_dwarf(dwarf)?;

let mut frames = ctx.find_frames(addr)?.skip_all_loads()?;
while let Some(frame) = frames.next()? {
    let func = frame.function.as_ref();
    let name = func.map(|f| f.demangle()).transpose()?;   // Cow<str>, already demangled
    let raw  = func.map(|f| f.raw_name()).transpose()?;
    if let Some(loc) = &frame.location {
        (loc.file, loc.line, loc.column);
    }
    frame.dw_die_offset;   // back-reference into the DIE tree
}
```

**Frames come innermost-first**: the most deeply inlined frame is yielded first, the physical
function last. That ordering is what you render as a nested stack.

**`find_frames` returns a `LookupResult`, not frames.** This is the split-DWARF
accommodation: the lookup may need to load a `.dwo` before it can finish.

```rust
// If you don't support split DWARF:
let frames = ctx.find_frames(addr)?.skip_all_loads()?;

// If you do:
let mut lookup = ctx.find_frames(addr)?;
let frames = loop {
    match lookup {
        LookupResult::Output(frames) => break frames?,
        LookupResult::Load { load, continuation } => {
            let dwo = load_dwo(&load)?;              // your loader
            lookup = continuation.resume(dwo);
        }
    }
};
```

Getting this wrong on a split-DWARF build means silently losing inline frames.

### 3.2 Cost model

`Context::new` parses and indexes DWARF. On a large binary this is hundreds of milliseconds to
seconds and allocates substantially. Lookups afterwards are fast.

**Build one `Context` per binary, cache it keyed by build ID, and never construct it in a
loop.** For Phase 3, where you symbolize millions of samples, this is the difference between
seconds and hours. Also memoize `addr -> Vec<Frame>` — profile samples cluster heavily, and a
simple `HashMap` cache typically hits over 95%.

`ctx.find_location(addr)` is cheaper when you only need the innermost line and do not care
about the inline chain. `ctx.find_location_range(start, end)` is the right call for
attributing a whole address range at once.

### 3.3 What it does not do

- It gives you names and lines, not variables. Variable recovery is your own `gimli` work
  (§2.4).
- Its `demangle()` handles Rust and C++, but you want `rustc-demangle` directly when you need
  the *structure* rather than a display string (§4).

---

## 4. `rustc-demangle`

### 4.1 The API is smaller than you want

```rust
use rustc_demangle::{demangle, try_demangle};

let d = demangle("_RNvNtCs1234_5mycrate6module8function");
format!("{}", d);      // full, with hash suffix on legacy symbols
format!("{:#}", d);    // "alternate": legacy hash suffix omitted
d.as_str();            // the original mangled string

try_demangle(s)?;      // errors instead of passing through unrecognised input
```

**`demangle()` never fails.** Given a non-Rust symbol it returns something that formats back to
the input. Use `try_demangle` when you need to know whether a symbol is Rust at all — for
example to separate C dependencies from Rust code in size attribution.

**The crate gives you a `Display` impl, not a parse tree.** This is the single most important
practical fact about it, because Phase 1's whole thesis requires structure.

### 4.2 The two mangling schemes

**Legacy** (`_ZN...17h<16 hex digits>E`): a length-prefixed path plus a hash of the symbol's
full definition including generic arguments. **Generic arguments are not recoverable** — two
instantiations of the same function differ only in an opaque hash.

**v0** (`_R...`): structured. Encodes crate with disambiguator, nested path components, impl
blocks, and **generic arguments explicitly**, with backreferences for compression.

Consequence: **monomorphization grouping only works under v0.** Under legacy you can group by
path but not distinguish instantiation from overload, and the demangled forms of twelve
instantiations look identical apart from the hash. Detect the scheme and degrade honestly in
the UI rather than presenting worse results as equivalent.

Enable it: `-Csymbol-mangling-version=v0`, or `[profile.x] rustflags` / `RUSTFLAGS`.

### 4.3 v0 grammar, the parts you need

```
_R <path>
   C <disambiguator?> <ident>          crate root
   N <ns> <path> <disambiguator?> <ident>   nested (ns: 'v' value, 't' type, 'C' closure…)
   M <impl-path> <type>                inherent impl
   X <impl-path> <type> <trait>        trait impl
   I <path> <generic-arg>* E           GENERIC INSTANTIATION
   B <backref>                         backreference to an earlier substring
```

Basic types are single letters: `a` i8, `b` bool, `c` char, `d` f64, `e` str, `f` f32,
`h` u8, `i` isize, `j` usize, `l` i32, `m` u32, `n` i128, `o` u128, `s` i16, `t` u16,
`u` unit, `x` i64, `y` u64, `z` `!`, `p` placeholder `_`.

**`I ... E` is the whole game.** A symbol containing `I` at the outermost path level is a
monomorphized instance; the path before `I` is the generic origin, and the args between `I`
and `E` are what distinguishes instances.

### 4.4 Grouping without a parser

Since the crate does not expose the tree, there are three routes:

| Route | Verdict |
|---|---|
| Parse v0 yourself | Correct and reusable. Perhaps 400-600 lines, mostly mechanical, needs backreference support. The right answer if Phase 1 is core to the product. |
| String surgery on demangled output | Fast to write, adequate to start, fragile. |
| Group on the mangled form | Actually quite good — see below. |

**The pragmatic algorithm — normalize the mangled string:**

```rust
/// Group key = the mangled symbol with outermost generic-argument groups elided.
/// `_RINvNt...fooIlEE` and `_RINvNt...fooIyEE` collapse to the same key.
fn generic_origin_key(mangled: &str) -> Option<String> {
    // Scan the v0 body tracking nesting; on encountering `I` at depth 0 of the
    // final path component, emit `I<>` and skip to the matching `E`.
    // Backreferences (`B`) must be resolved first, or two textually different
    // symbols with the same structure will fail to group.
}
```

**Backreferences are the catch.** v0 compresses repeated substrings with `B<base62>` pointing
at an earlier offset in the *same* symbol. Two symbols that are structurally identical can be
textually different because one compressed a repeat and the other did not. Any string-level
grouping must expand backreferences first, which is already most of a parser — which is the
argument for just writing the parser.

**Recommendation:** write the v0 parser. It is a contained, well-specified, testable piece of
work with a documented grammar, it is the foundation of the product's most differentiated
feature, and half-measures will produce grouping bugs you cannot explain to users. Property-
test it by round-tripping against `rustc-demangle`'s `Display` output on every symbol in the
corpus binaries: if your parser's re-rendering does not match, you have a bug.

### 4.5 Symbol roles

Classification for size attribution (`Finding::SizeDriver` categories). Recognisable patterns:

| Category | Signal |
|---|---|
| `Drop` glue | path ends in `drop_in_place` |
| Vtables | `DW_AT_name` / symbol contains `vtable`; also `.data.rel.ro` residency |
| Formatting | paths under `core::fmt`, `Display::fmt`, `Debug::fmt` instantiations |
| Panic machinery | `core::panicking::*`, `unwrap_failed`, `panic_bounds_check` |
| Unwinding tables | `.eh_frame` / `.gcc_except_table` section bytes, not symbols |
| Panic strings | `.rodata` referenced only from panic paths (needs relocation analysis) |
| Your code vs deps | crate name in the path root, matched against `cargo_metadata` |

The last one is the most valuable in the UI and the easiest: v0 encodes the crate root
directly, and `cargo_metadata` tells you which crates are workspace members.

---

## 5. `iced-x86`

### 5.1 Decoding

```rust
use iced_x86::{Decoder, DecoderOptions, Instruction, InstructionInfoFactory,
               Formatter, IntelFormatter, OpAccess, Register, FlowControl, OpKind};

let mut decoder = Decoder::with_ip(64, code_bytes, section_vaddr, DecoderOptions::NONE);
let mut instr = Instruction::default();
let mut info_factory = InstructionInfoFactory::new();

while decoder.can_decode() {
    decoder.decode_out(&mut instr);          // reuses the allocation
    if instr.is_invalid() { /* data in .text, or a decode desync */ }

    instr.ip();
    instr.len();
    instr.mnemonic();
    instr.code();                             // exact instruction variant
    instr.flow_control();                     // Next | Call | UnconditionalBranch | Return | ...
    instr.near_branch_target();               // resolved absolute target for jmp/call
    instr.is_ip_rel_memory_operand();
    instr.ip_rel_memory_address();            // resolved RIP-relative data address
}
```

`decode_out` into a reused `Instruction` avoids per-instruction allocation. On a large `.text`
this matters.

**Decode desync.** Jump tables, alignment padding, and constant pools live inside `.text`. A
linear sweep will decode garbage. Mitigations, in increasing order of effort: bound decoding to
one function's DWARF range; use symbol boundaries; follow control flow recursively rather than
sweeping linearly. For Binmap, function-bounded decoding is sufficient — you always know
which function you are looking at.

### 5.2 `InstructionInfoFactory` — why this crate

This is the API that decides the crate choice.

```rust
let info = info_factory.info(&instr);

for reg_use in info.used_registers() {
    reg_use.register();     // Register::RAX
    reg_use.access();       // OpAccess::Read | Write | ReadWrite | CondRead | CondWrite | ...
}

for mem_use in info.used_memory() {
    mem_use.base();         // Register
    mem_use.index();
    mem_use.scale();
    mem_use.displacement();
    mem_use.memory_size();
    mem_use.access();
}
```

`used_registers()` accounts for implicit operands — `push` touching `rsp`, `div` touching
`rdx:rax`, string instructions touching `rsi`/`rdi`/`rcx`, flag reads and writes. Maintaining
those tables by hand is a multi-month mistake.

### 5.3 The backward slice

This is the concrete implementation of `DESIGN.md` §5.4 — "which instructions could have
written this value?" It is the algorithm that keeps the model's context small and grounded.

```rust
/// Walk backwards from `pc`, following data dependencies for `target`.
fn backward_slice(
    instrs: &[Instruction],       // decoded, ascending, within one function
    info: &mut InstructionInfoFactory,
    pc_index: usize,
    target: Register,
    budget: usize,
) -> Vec<usize> {
    let mut tracked: HashSet<Register> = [full_register(target)].into();
    let mut slice = Vec::new();

    for i in (0..pc_index).rev() {
        if slice.len() >= budget { break; }
        let inf = info.info(&instrs[i]);

        let writes_tracked = inf.used_registers().iter().any(|u| {
            matches!(u.access(), OpAccess::Write | OpAccess::ReadWrite | OpAccess::CondWrite)
                && tracked.contains(&full_register(u.register()))
        });

        if writes_tracked {
            slice.push(i);
            // The value now depends on this instruction's inputs instead.
            for u in inf.used_registers() {
                if matches!(u.access(), OpAccess::Read | OpAccess::ReadWrite | OpAccess::CondRead) {
                    tracked.insert(full_register(u.register()));
                }
            }
            for m in inf.used_memory() {
                if m.base() != Register::None { tracked.insert(full_register(m.base())); }
                if m.index() != Register::None { tracked.insert(full_register(m.index())); }
            }
            // A full (non-conditional) overwrite kills the old dependency.
            if is_full_overwrite(&instrs[i], target) {
                tracked.remove(&full_register(target));
            }
        }
    }
    slice.reverse();
    slice
}
```

**Sub-register aliasing is mandatory.** `Register::EAX`, `AX`, `AL`, and `AH` all alias `RAX`.
`iced-x86` provides `Register::full_register()` and `full_register32()`; normalize through it
or your slice will miss the instruction that actually wrote the value. Note the x86-64 quirk
that a 32-bit write zero-extends into the full 64-bit register while an 8- or 16-bit write does
not — relevant if you model partial writes precisely.

This function is pure, takes no context, and is trivially unit-testable against hand-written
instruction sequences. Test it that way.

### 5.4 Formatting with symbol resolution

```rust
use iced_x86::{SymbolResolver, SymbolResult, IntelFormatter, GasFormatter, FormatterOutput};

struct BinmapSymbols { by_addr: BTreeMap<u64, String> }

impl SymbolResolver for BinmapSymbols {
    fn symbol(&mut self, _instr: &Instruction, _op: u32, _instr_op: Option<u32>,
              address: u64, _size: u32) -> Option<SymbolResult<'_>> {
        self.by_addr.get(&address).map(|s| SymbolResult::with_str(address, s.as_str()))
    }
}

let mut fmt = IntelFormatter::with_options(Some(Box::new(resolver)), None);
fmt.options_mut().set_first_operand_char_index(10);
fmt.options_mut().set_space_after_operand_separator(true);
fmt.options_mut().set_rip_relative_addresses(true);
```

Formatters available: Intel, `GasFormatter` (AT&T), masm, nasm, plus a `FastFormatter` when
throughput matters more than options. Offer Intel and AT&T in the UI — the audience is split
and it is a one-line switch.

`set_rip_relative_addresses(true)` shows resolved absolute addresses for RIP-relative operands,
which is how you turn `lea rax, [rip+0x1234]` into a clickable reference to a `.rodata` string.
That directly serves the "where did this panic message come from" question.

Implement `FormatterOutput` rather than formatting to `String` when you want token kinds for
syntax highlighting — it hands you each token with a `FormatterTextKind` (mnemonic, register,
number, punctuation), which feeds the disassembly pane's highlighter directly and removes the
need for a tree-sitter asm grammar.

### 5.5 `capstone`, briefly

Choose it only if you need non-x86 architectures. It brings a C dependency and a build
requirement, its Rust bindings expose a thinner slice of instruction detail, and its
`RegAccess` information is less complete than `InstructionInfoFactory`. Since `rr` pins you to
x86-64 for Phase 4 anyway, the portability argument does not apply.

`yaxpeax-x86` is the other credible pure-Rust option, with a cleaner semantic model, but a
smaller ecosystem and no equivalent formatter flexibility.

---

## 6. Cross-cutting invariants

Assertions worth encoding as tests, because each catches a class of silent wrongness:

1. **Attributed bytes + padding + section overhead == file size.** Reconciles with `bloaty`.
2. **Every `PT_LOAD` address in a core resolves to a known mapping** from `NT_FILE`.
3. **Load bias derived from `NT_FILE` == load bias derived from `AT_PHDR`.**
4. **`addr2line` symbolization of a known address matches the `addr2line(1)` binary** on the
   golden test binaries.
5. **v0 parser round-trip:** re-rendering a parsed symbol equals `rustc-demangle`'s output.
6. **Decoded instruction lengths sum to exactly the function's DWARF range length.** A
   mismatch means a decode desync or a wrong range.
7. **Every DWARF register number used in a location list has a mapping** in
   `DWARF_TO_PRSTATUS`. Fail loudly on an unmapped one rather than reading index 0.
