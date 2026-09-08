//! TOOL.md section 6.7: the LD_PRELOAD cdylib. Path virtualization and ownership
//! virtualization, which are two jobs and not one.
//!
//! Empty. The work is in `TODO/interpose.md`.
//!
//! Build constraints that apply to every line added here, none of them
//! optional, because this object runs inside other people's processes:
//! a version script exporting only the interposed symbols; no allocation and
//! no locks on an interposed path; no `println!`, write to fd 2 directly; it
//! must survive the payload forking.
#![forbid(unsafe_op_in_unsafe_fn)]
