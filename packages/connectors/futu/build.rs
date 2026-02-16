use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let proto_dir = manifest_dir.join("proto");
    let protos: [&str; 22] = [
        "Common.proto",
        "InitConnect.proto",
        "KeepAlive.proto",
        "GetGlobalState.proto",
        "Qot_Common.proto",
        "Qot_Sub.proto",
        "Qot_RegQotPush.proto",
        "Qot_GetBasicQot.proto",
        "Qot_UpdateBasicQot.proto",
        "Qot_GetKL.proto",
        "Qot_UpdateKL.proto",
        "Trd_Common.proto",
        "Trd_GetAccList.proto",
        "Trd_UnlockTrade.proto",
        "Trd_SubAccPush.proto",
        "Trd_GetFunds.proto",
        "Trd_GetPositionList.proto",
        "Trd_GetOrderList.proto",
        "Trd_PlaceOrder.proto",
        "Trd_ModifyOrder.proto",
        "Trd_UpdateOrder.proto",
        "Trd_UpdateOrderFill.proto",
    ];

    let proto_files: Vec<PathBuf> = protos.iter().map(|p| proto_dir.join(p)).collect();
    for path in proto_files.iter() {
        if !path.exists() {
            return Err(format!("missing proto file: {}", path.display()).into());
        }
    }

    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", protoc);

    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?).join("futu_pb");
    // `prost-build` does not reliably create nested output directories.
    std::fs::create_dir_all(&out_dir)?;

    let mut config = prost_build::Config::new();
    config.out_dir(out_dir);
    config.include_file("mod.rs");
    config.compile_protos(&proto_files, &[proto_dir])?;

    for p in protos.iter() {
        println!("cargo:rerun-if-changed=proto/{p}");
    }
    Ok(())
}
