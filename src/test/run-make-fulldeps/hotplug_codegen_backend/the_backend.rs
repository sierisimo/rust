#![feature(rustc_private)]

extern crate rustc;
extern crate rustc_codegen_utils;
#[macro_use]
extern crate rustc_data_structures;
extern crate rustc_hir;
extern crate rustc_target;
extern crate rustc_driver;
extern crate rustc_session;
extern crate rustc_span;

use std::any::Any;
use std::sync::Arc;
use std::path::Path;
use rustc::ty::TyCtxt;
use rustc::ty::query::Providers;
use rustc::middle::cstore::{EncodedMetadata, MetadataLoader, MetadataLoaderDyn};
use rustc::dep_graph::DepGraph;
use rustc::util::common::ErrorReported;
use rustc_codegen_utils::codegen_backend::CodegenBackend;
use rustc_data_structures::sync::MetadataRef;
use rustc_data_structures::owning_ref::OwningRef;
use rustc_session::Session;
use rustc_session::config::OutputFilenames;
use rustc_span::symbol::Symbol;
use rustc_target::spec::Target;

pub struct NoLlvmMetadataLoader;

impl MetadataLoader for NoLlvmMetadataLoader {
    fn get_rlib_metadata(&self, _: &Target, filename: &Path) -> Result<MetadataRef, String> {
        let buf = std::fs::read(filename).map_err(|e| format!("metadata file open err: {:?}", e))?;
        let buf: OwningRef<Vec<u8>, [u8]> = OwningRef::new(buf);
        Ok(rustc_erase_owner!(buf.map_owner_box()))
    }

    fn get_dylib_metadata(&self, target: &Target, filename: &Path) -> Result<MetadataRef, String> {
        self.get_rlib_metadata(target, filename)
    }
}

struct TheBackend;

impl CodegenBackend for TheBackend {
    fn metadata_loader(&self) -> Box<MetadataLoaderDyn> {
        Box::new(NoLlvmMetadataLoader)
    }

    fn provide(&self, providers: &mut Providers) {
        rustc_codegen_utils::symbol_names::provide(providers);

        providers.target_features_whitelist = |tcx, _cnum| {
            tcx.arena.alloc(Default::default()) // Just a dummy
        };
        providers.is_reachable_non_generic = |_tcx, _defid| true;
        providers.exported_symbols = |_tcx, _crate| Arc::new(Vec::new());
    }

    fn provide_extern(&self, providers: &mut Providers) {
        providers.is_reachable_non_generic = |_tcx, _defid| true;
    }

    fn codegen_crate<'a, 'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        _metadata: EncodedMetadata,
        _need_metadata_module: bool,
    ) -> Box<dyn Any> {
        use rustc_hir::def_id::LOCAL_CRATE;

        Box::new(tcx.crate_name(LOCAL_CRATE) as Symbol)
    }

    fn join_codegen(
        &self,
        ongoing_codegen: Box<dyn Any>,
        _sess: &Session,
        _dep_graph: &DepGraph,
    ) -> Result<Box<dyn Any>, ErrorReported> {
        let crate_name = ongoing_codegen.downcast::<Symbol>()
            .expect("in join_codegen: ongoing_codegen is not a Symbol");
        Ok(crate_name)
    }

    fn link(
        &self,
        sess: &Session,
        codegen_results: Box<dyn Any>,
        outputs: &OutputFilenames,
    ) -> Result<(), ErrorReported> {
        use std::io::Write;
        use rustc_session::config::CrateType;
        use rustc_codegen_utils::link::out_filename;
        let crate_name = codegen_results.downcast::<Symbol>()
            .expect("in link: codegen_results is not a Symbol");
        for &crate_type in sess.opts.crate_types.iter() {
            if crate_type != CrateType::Rlib {
                sess.fatal(&format!("Crate type is {:?}", crate_type));
            }
            let output_name =
                out_filename(sess, crate_type, &outputs, &*crate_name.as_str());
            let mut out_file = ::std::fs::File::create(output_name).unwrap();
            write!(out_file, "This has been \"compiled\" successfully.").unwrap();
        }
        Ok(())
    }
}

/// This is the entrypoint for a hot plugged rustc_codegen_llvm
#[no_mangle]
pub fn __rustc_codegen_backend() -> Box<dyn CodegenBackend> {
    Box::new(TheBackend)
}
