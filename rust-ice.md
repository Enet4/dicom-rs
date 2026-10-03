# Rust ICE report

Reproduced in 2026-10-03 with the following tooling and system:

```none
cargo 1.99.0 (5f94df478 2026-08-27)
release: 1.99.0
commit-hash: 5f94df4789f005f9a352888e8355ffc645b7ed0e
commit-date: 2026-08-27
host: x86_64-unknown-linux-gnu
libgit2: 1.9.6 (sys:0.21.0 vendored)
libcurl: 8.21.0-DEV (sys:0.4.90+curl-8.21.0 vendored ssl:OpenSSL/3.6.3)
ssl: OpenSSL 3.6.3 9 Jun 2026
os: Manjaro 26.1.2 (Bian-May) [64-bit]
```

## steps to reproduce

```sh
git checkout ice/storescu/unstable-fingerprint~1
cargo build -p dicom-storescu --features=tls
git checkout ice/storescu/unstable-fingerprint
RUST_BACKTRACE=1 cargo build -p dicom-storescu --features=tls
```

## console output

```log
❯ RUST_BACKTRACE=1 cargo build -p dicom-storescu --features=tls
   Compiling dicom-storescu v0.10.0 (.../dicom-rs/storescu)
error[E0308]: mismatched types
   --> storescu/src/main.rs:574:17
    |
563 |             let mut scu_options = get_scu_options(
    |                                   --------------- arguments to this function are incorrect
...
574 |                 tls_config_clone,
    |                 ^^^^^^^^^^^^^^^^ expected `Option<ClientConfig>`, found `ClientConfig`
    |
    = note: expected enum `Option<ClientConfig>`
             found struct `ClientConfig`
note: function defined here
   --> storescu/src/main.rs:191:15
    |
191 | pub(crate) fn get_scu_options<'a>(
    |               ^^^^^^^^^^^^^^^
...
201 |     #[cfg(feature = "tls")] tls_options: Option<rustls::ClientConfig>,
    |     -----------------------------------------------------------------
help: try wrapping the expression in `Some`
    |
574 |                 Some(tls_config_clone),
    |                 +++++                +

error: internal compiler error: encountered incremental compilation error with evaluate_obligation(d40445186fc295cc-99705675039751b2)
  |
  = note: please follow the instructions below to create a bug report with the provided information
  = note: for incremental compilation bugs, having a reproduction is vital
  = note: an ideal reproduction consists of the code before and some patch that then triggers the bug when applied and compiled again
  = note: as a workaround, you can run `cargo clean -p dicom_storescu` or `cargo clean` to allow your project to compile


thread 'rustc' (190663) panicked at /rustc-dev/b940084d7eb6a299eb4bfeb8e34901bc051e7ac4/compiler/rustc_middle/src/verify_ich.rs:82:9:
Found unstable fingerprints for evaluate_obligation(d40445186fc295cc-99705675039751b2): Ok(EvaluatedToOk)
stack backtrace:
   0: __rustc::rust_begin_unwind
   1: core::panicking::panic_fmt
   2: rustc_middle::verify_ich::incremental_verify_ich_failed
   3: rustc_middle::verify_ich::incremental_verify_ich::<rustc_middle::query::erase::ErasedData<[u8; 2]>>
   4: rustc_query_impl::execution::try_execute_query::<rustc_middle::query::caches::DefaultCache<rustc_type_ir::canonical::CanonicalQueryInput<rustc_middle::ty::context::TyCtxt, rustc_middle::ty::ParamEnvAnd<rustc_middle::ty::predicate::Predicate>>, rustc_middle::query::erase::ErasedData<[u8; 2]>>, true>
   5: <rustc_trait_selection::traits::fulfill::FulfillProcessor as rustc_data_structures::obligation_forest::ObligationProcessor>::process_obligation
   6: <rustc_data_structures::obligation_forest::ObligationForest<rustc_trait_selection::traits::fulfill::PendingPredicateObligation>>::process_obligations::<rustc_trait_selection::traits::fulfill::FulfillProcessor>
   7: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_argument_types
   8: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_method_call
   9: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_with_expectation_and_args
  10: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_block
  11: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_with_expectation_and_args
  12: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_match
  13: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_with_expectation_and_args
  14: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_block
  15: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_with_expectation_and_args
  16: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_match
  17: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_with_expectation_and_args
  18: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_with_expectation_and_args
  19: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_block
  20: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_with_expectation_and_args
  21: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_with_expectation_and_args
  22: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_block
  23: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_with_expectation_and_args
  24: rustc_hir_typeck::check::check_fn
  25: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_closure
  26: <rustc_hir_typeck::fn_ctxt::FnCtxt>::check_expr_with_expectation_and_args
  27: rustc_hir_typeck::check::check_fn
  28: rustc_hir_typeck::typeck_with_inspect::{closure#0}
      [... omitted 8 frames ...]
  29: rustc_hir_analysis::collect::type_of::opaque::find_opaque_ty_constraints_for_rpit
  30: rustc_hir_analysis::collect::type_of::type_of_opaque
  31: rustc_query_impl::execution::try_execute_query::<rustc_middle::query::caches::DefIdCache<rustc_middle::query::erase::ErasedData<[u8; 8]>>, true>
  32: rustc_hir_analysis::collect::type_of::type_of
      [... omitted 1 frame ...]
  33: rustc_hir_analysis::check::check::check_opaque
  34: rustc_hir_analysis::check::check::check_item_type
  35: rustc_hir_analysis::check::wfcheck::check_well_formed
      [... omitted 1 frame ...]
  36: <rustc_middle::hir::ModuleItems>::par_opaques::<rustc_hir_analysis::check::wfcheck::check_type_wf::{closure#5}>
  37: rustc_hir_analysis::check::wfcheck::check_type_wf
      [... omitted 1 frame ...]
  38: rustc_hir_analysis::check_crate
  39: rustc_interface::passes::analysis
  40: rustc_query_impl::execution::try_execute_query::<rustc_middle::query::caches::SingleCache<rustc_middle::query::erase::ErasedData<[u8; 0]>>, true>
  41: rustc_interface::interface::run_compiler::<(), rustc_driver_impl::run_compiler::{closure#0}>::{closure#2}
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.

error: the compiler unexpectedly panicked. This is a bug

note: we would appreciate a bug report: https://github.com/rust-lang/rust/issues/new?labels=C-bug%2C+I-ICE%2C+T-compiler&template=ice.md

note: rustc 1.99.0 (b940084d7 2026-09-28) running on x86_64-unknown-linux-gnu

note: compiler flags: --crate-type bin -C embed-bitcode=no -C debuginfo=2 -C incremental=[REDACTED]

note: some of the compiler flags provided by cargo are hidden

query stack during panic:
#0 [evaluate_obligation] evaluating trait selection obligation `dicom_ul::pdu::writer::WriteChunkError: core::marker::Send`
#1 [typeck_root] type-checking `run_async`
#2 [mir_borrowck] borrow-checking `run_async`
#3 [type_of_opaque] computing type of opaque `run_async::{opaque#0}`
#4 [type_of] computing type of `run_async::{opaque#0}`
#5 [check_well_formed] checking that `run_async::{opaque#0}` is well-formed
#6 [check_type_wf] checking that types are well-formed
#7 [analysis] running analysis passes on crate `dicom_storescu`
end of query stack
there was a panic while trying to force a dep node
try_mark_green dep node stack:
#0 check_match(dicom_storescu[0ec9]::run_async)
#1 mir_built(dicom_storescu[0ec9]::run_async)
#2 has_ffi_unwind_calls(dicom_storescu[0ec9]::run_async)
#3 mir_promoted(dicom_storescu[0ec9]::run_async)
#4 mir_borrowck(dicom_storescu[0ec9]::run_async)
end of try_mark_green dep node stack
For more information about this error, try `rustc --explain E0308`.
error: could not compile `dicom-storescu` (bin "dicom-storescu") due to 2 previous errors
```

