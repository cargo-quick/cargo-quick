#[cfg(test)]
mod tests {
    use cargo::core::TargetKind;

    use super::super::*;
    #[test]
    fn libnghttp2_sys_structure() -> Result<()> {
        let config = Config::default()?;
        let ws = Workspace::new(&Path::new("Cargo.toml").canonicalize()?, &config)?;
        let options = CompileOptions::new(&config, CompileMode::Build)?;
        let interner = UnitInterner::new();
        let bcx = create_bcx(&ws, &options, &interner)?;

        let [compile_build_script_unit]: [_; 1] = bcx
            .unit_graph
            .filter_by_name("libnghttp2-sys")
            // FIXME: turn this into an extension method on Unit or something.
            .filter(|unit| {
                unit.target.name() == "build-script-build"
                    && unit
                        .target
                        .src_path()
                        .path()
                        .map(|path| path.ends_with("build.rs"))
                        .unwrap_or_default()
                    && unit.mode == CompileMode::Build
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let [run_build_script_unit]: [_; 1] = bcx
            .unit_graph
            .filter_by_name("libnghttp2-sys")
            // FIXME: turn this into an extension method on Unit or something.
            .filter(|unit| {
                unit.target.name() == "build-script-build"
                    && unit
                        .target
                        .src_path()
                        .path()
                        .map(|path| path.ends_with("build.rs"))
                        .unwrap_or_default()
                    && unit.mode == CompileMode::RunCustomBuild
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let [compile_lib_script_unit]: [_; 1] = bcx
            .unit_graph
            .filter_by_name("libnghttp2-sys")
            // FIXME: turn this into an extension method on Unit or something.
            .filter(|unit| {
                unit.target.name() == "libnghttp2-sys"
                    && matches!(unit.target.kind(), TargetKind::Lib(_))
                    && unit.mode == CompileMode::Build
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        assert_ne!(compile_build_script_unit, run_build_script_unit);
        assert_ne!(run_build_script_unit, compile_lib_script_unit);
        assert_ne!(compile_lib_script_unit, compile_build_script_unit);
        Ok(())
    }
}
