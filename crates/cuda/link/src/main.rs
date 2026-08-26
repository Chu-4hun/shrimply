use std::{env, fs};

fn main() {
    let mut args = env::args().skip(1);
    let input = args.next().expect("missing input LTOIR path");
    let output = args.next().expect("missing output cubin path");
    let name = args.next().expect("missing module name");
    let target = args.next().expect("missing CUDA target");
    assert!(args.next().is_none(), "unexpected extra argument");

    let ltoir = fs::read(&input).expect("read CUDA LTOIR");
    let cubin = cuda_host::ltoir::link_ltoir_to_cubin_with_options(
        &ltoir,
        &name,
        &target,
        true,
    )
    .expect("link CUDA LTOIR to cubin");
    fs::write(output, cubin).expect("write CUDA cubin");
}
