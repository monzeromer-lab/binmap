//! A program that dies in known ways at known lines.
//!
//! Phase 2's acceptance criterion needs "20 crashes with known ground truth",
//! and ground truth here means the line number is written down in the source
//! next to the crash. Each site is a separate function so the faulting line is
//! unambiguous, and `SITES` records what each one should resolve to — a table
//! the harness reads rather than a list anyone has to keep in sync by hand.
//!
//! The shapes are deliberately varied, because an unwinder that handles one
//! kind of frame and not another is not a working unwinder: deep recursion,
//! generics, inlining, trait objects, closures, iterator chains and drop glue
//! all put different things on the stack.
//!
//! Each site also faults at a *distinct* offset from null. That is not
//! decoration: with identical bodies the compiler folded several of these into
//! one function, so the corpus claimed twenty crash sites and contained
//! thirteen — and the acceptance harness dutifully reported the wrong ground
//! truth for the folded ones, because the address genuinely belonged to both.

use std::env;

/// Every crash site, and the line its innermost frame should resolve to.
///
/// The line numbers are asserted by `binmap-eval acceptance2`, so a stale
/// entry here fails the phase's own criterion rather than passing quietly.
pub const SITES: &[(&str, u32)] = &[
    ("null_write", 53),
    ("null_read", 61),
    ("null_field", 75),
    ("wild_pointer", 83),
    ("deep_recursion", 98),
    ("generic_null", 106),
    ("inlined_null", 115),
    ("closure_null", 128),
    ("trait_object_null", 144),
    ("iterator_null", 160),
    ("drop_glue_null", 172),
    ("nested_generic_null", 187),
    ("array_out_of_bounds", 201),
    ("unaligned_write", 209),
    ("null_function_pointer", 222),
    ("slice_from_null", 236),
    ("vec_from_null", 245),
    ("string_from_null", 254),
    ("boxed_null", 262),
    ("double_indirect_null", 271),
];

#[inline(never)]
fn null_write() {
    unsafe {
        let pointer: *mut u64 = std::ptr::null_mut();
        pointer.byte_add(8).write_volatile(1); // site: null_write
    }
}

#[inline(never)]
fn null_read() -> u64 {
    unsafe {
        let pointer: *const u64 = std::ptr::null();
        pointer.byte_add(16).read_volatile() // site: null_read
    }
}

#[repr(C)]
struct Wide {
    _padding: [u64; 64],
    field: u64,
}

#[inline(never)]
fn null_field() -> u64 {
    unsafe {
        let pointer: *const Wide = std::ptr::null();
        std::ptr::read_volatile(&raw const (*pointer).field) // site: null_field
    }
}

#[inline(never)]
fn wild_pointer() -> u64 {
    unsafe {
        let pointer = 0xdead_0000_usize as *const u64;
        pointer.byte_add(32).read_volatile() // site: wild_pointer
    }
}

#[inline(never)]
fn deep_recursion(depth: u32) -> u64 {
    if depth > 0 {
        // LLVM turns this into a loop whatever shape it is written in — a
        // tail call directly, and an accumulator pattern through its
        // tail-recursion pass — so this site tests a *faulting* function
        // rather than a deep stack. `explicit_panic` is the one that actually
        // recurses on the stack, because its `-> !` return type defeats both.
        return deep_recursion(depth - 1) + 1;
    }
    unsafe {
        let pointer: *const u64 = std::ptr::null();
        pointer.byte_add(40).read_volatile() // site: deep_recursion
    }
}

#[inline(never)]
fn generic_null<T: Copy + Default>() -> T {
    unsafe {
        let pointer: *const T = std::ptr::null();
        pointer.byte_add(48).read_volatile() // site: generic_null
    }
}

/// Inlined into its caller, so the faulting address has two source locations.
#[inline(always)]
fn always_inlined() -> u64 {
    unsafe {
        let pointer: *const u64 = std::ptr::null();
        pointer.byte_add(56).read_volatile() // site: inlined_null
    }
}

#[inline(never)]
fn inlined_null() -> u64 {
    always_inlined()
}

#[inline(never)]
fn closure_null() -> u64 {
    let closure = || unsafe {
        let pointer: *const u64 = std::ptr::null();
        pointer.byte_add(64).read_volatile() // site: closure_null
    };
    closure()
}

trait Work {
    fn perform(&self) -> u64;
}

struct Worker;

impl Work for Worker {
    #[inline(never)]
    fn perform(&self) -> u64 {
        unsafe {
            let pointer: *const u64 = std::ptr::null();
            pointer.byte_add(72).read_volatile() // site: trait_object_null
        }
    }
}

#[inline(never)]
fn trait_object_null() -> u64 {
    let worker: Box<dyn Work> = Box::new(Worker);
    worker.perform()
}

#[inline(never)]
fn iterator_null() -> u64 {
    (0..4u64)
        .map(|_| unsafe {
            let pointer: *const u64 = std::ptr::null();
            pointer.byte_add(80).read_volatile() // site: iterator_null
        })
        .sum()
}

struct Exploding;

impl Drop for Exploding {
    #[inline(never)]
    fn drop(&mut self) {
        unsafe {
            let pointer: *mut u64 = std::ptr::null_mut();
            pointer.byte_add(88).write_volatile(2); // site: drop_glue_null
        }
    }
}

#[inline(never)]
fn drop_glue_null() {
    let _exploding = Exploding;
}

#[inline(never)]
fn nested_generic_null<T: Copy>(value: T) -> T {
    unsafe {
        let pointer: *const T = std::ptr::null();
        let _ = value;
        pointer.byte_add(96).read_volatile() // site: nested_generic_null
    }
}

#[inline(never)]
fn array_out_of_bounds(index: usize) -> u64 {
    let array = std::hint::black_box([1u64, 2, 3, 4]);
    // Opaque, or the optimiser proves the offset is out of bounds, calls the
    // whole thing UB and deletes it — and the process exits 0 having never
    // faulted. A merely large offset also landed inside something else the
    // process had mapped and read it successfully, which is exactly why
    // out-of-bounds reads are dangerous rather than merely wrong.
    let index = std::hint::black_box(index);
    unsafe {
        array.as_ptr().byte_add(index).read_volatile() // site: array_out_of_bounds
    }
}

#[inline(never)]
fn unaligned_write() {
    unsafe {
        let pointer = 0x1_usize as *mut u64;
        pointer.write_volatile(3); // site: unaligned_write
    }
}

#[inline(never)]
fn null_function_pointer() {
    unsafe {
        // Read the address through a volatile load so the optimiser cannot
        // see it is zero. Transmuting a literal 0 let LLVM prove the call
        // unreachable and replace the whole function with an infinite loop,
        // so the process hung instead of faulting.
        let address = std::ptr::read_volatile(&raw const NOWHERE);
        let function: fn() = std::mem::transmute::<usize, fn()>(address);
        function(); // site: null_function_pointer
    }
}

/// Zero, in a place the optimiser has to read rather than assume.
static NOWHERE: usize = 0;

#[inline(never)]
fn slice_from_null() -> u64 {
    // Deliberately *not* `slice::from_raw_parts(null, ..)`: rustc rejects that
    // outright now, correctly, because it is UB even unused. Offsetting a null
    // pointer and reading it is the same fault without the lint.
    unsafe {
        let pointer: *const u64 = std::ptr::null();
        pointer.byte_add(128).read_volatile() // site: slice_from_null
    }
}

#[inline(never)]
fn vec_from_null() -> u64 {
    unsafe {
        let pointer = std::ptr::null::<u64>();
        let mut total = 0u64;
        total += pointer.add(1).read_volatile(); // site: vec_from_null
        total
    }
}

#[inline(never)]
fn string_from_null() -> u8 {
    unsafe {
        let pointer: *const u8 = std::ptr::null();
        pointer.byte_add(144).read_volatile() // site: string_from_null
    }
}

#[inline(never)]
fn boxed_null() -> u64 {
    unsafe {
        let boxed: *const Box<u64> = std::ptr::null();
        let inner = std::ptr::read_volatile(&raw const *boxed); // site: boxed_null
        **std::mem::ManuallyDrop::new(inner)
    }
}

#[inline(never)]
fn double_indirect_null() -> u64 {
    unsafe {
        let outer: *const *const u64 = std::ptr::null();
        let inner = outer.read_volatile(); // site: double_indirect_null
        inner.read_volatile()
    }
}

/// Runs forever at a known depth, for taking a core from a live process.
#[inline(never)]
fn wait_to_be_dumped(depth: u32) {
    if depth > 0 {
        wait_to_be_dumped(depth - 1);
        return;
    }
    println!("ready");
    loop {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[inline(never)]
fn explicit_panic(depth: u32) -> ! {
    if depth > 0 {
        explicit_panic(depth - 1)
    } else {
        panic!("the crasher panicked on purpose, at a known line");
    }
}

fn main() {
    let how = env::args().nth(1).unwrap_or_else(|| "wait".to_string());
    // `black_box` so the optimiser cannot decide the results are unused and
    // delete the call that was supposed to fault.
    let keep = std::hint::black_box;

    match how.as_str() {
        "null_write" => null_write(),
        "null_read" => { keep(null_read()); },
        "null_field" => { keep(null_field()); },
        "wild_pointer" => { keep(wild_pointer()); },
        "deep_recursion" => { keep(deep_recursion(std::hint::black_box(8))); },
        "generic_null" => { keep(generic_null::<u64>()); },
        "inlined_null" => { keep(inlined_null()); },
        "closure_null" => { keep(closure_null()); },
        "trait_object_null" => { keep(trait_object_null()); },
        "iterator_null" => { keep(iterator_null()); },
        "drop_glue_null" => drop_glue_null(),
        "nested_generic_null" => { keep(nested_generic_null(7u64)); },
        "array_out_of_bounds" => { keep(array_out_of_bounds(1 << 40)); },
        "unaligned_write" => unaligned_write(),
        "null_function_pointer" => null_function_pointer(),
        "slice_from_null" => { keep(slice_from_null()); },
        "vec_from_null" => { keep(vec_from_null()); },
        "string_from_null" => { keep(u64::from(string_from_null())); },
        "boxed_null" => { keep(boxed_null()); },
        "double_indirect_null" => { keep(double_indirect_null()); },
        "panic" => explicit_panic(3),
        _ => wait_to_be_dumped(4),
    }
}
