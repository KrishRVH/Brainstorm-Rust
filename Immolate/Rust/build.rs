#![allow(clippy::panic)]

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/cuda/brainstorm_cuda.cu");
    println!("cargo:rerun-if-env-changed=BRAINSTORM_SKIP_CUDA_BUILD");
    println!("cargo:rerun-if-env-changed=BRAINSTORM_CUDA_ARCH");
    println!("cargo:rerun-if-env-changed=CUDAHOSTCXX");
    println!("cargo:rerun-if-env-changed=NVCC");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let ptx_path = out_dir.join("brainstorm_cuda.ptx");

    if env::var_os("BRAINSTORM_SKIP_CUDA_BUILD").is_some() {
        std::fs::write(&ptx_path, b"").expect("write empty CUDA PTX");
        return;
    }

    let nvcc = env::var("NVCC").unwrap_or_else(|_| "nvcc".to_owned());
    let ccbin = env::var("CUDAHOSTCXX").unwrap_or_else(|_| "gcc-12".to_owned());
    let arch = env::var("BRAINSTORM_CUDA_ARCH").unwrap_or_else(|_| "sm_89".to_owned());

    let status = Command::new(&nvcc)
        .args([
            "-ptx",
            "-arch",
            &arch,
            "-O3",
            "--std=c++17",
            "-lineinfo",
            "--fmad=false",
            "-Xptxas",
            "-fmad=false",
            "-prec-div=true",
            "-prec-sqrt=true",
            "-ccbin",
            &ccbin,
            "-o",
        ])
        .arg(&ptx_path)
        .arg("src/cuda/brainstorm_cuda.cu")
        .status();

    match status {
        Ok(status) if status.success() => {},
        Ok(status) => {
            panic!(
                "nvcc failed with status {status}. Install nvidia-cuda-toolkit or set BRAINSTORM_SKIP_CUDA_BUILD=1"
            );
        },
        Err(err) => {
            panic!(
                "failed to run {nvcc}: {err}. Install nvidia-cuda-toolkit or set BRAINSTORM_SKIP_CUDA_BUILD=1"
            );
        },
    }
}
