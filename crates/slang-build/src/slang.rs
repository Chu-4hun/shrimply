use std::{
    env, fs,
    fs::OpenOptions,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
};

const CONFIGURATION: &str = "Release";
const EXTENSION: &str = "slang";
const GLSLANG_PREFIX: &str = "libslang-glslang-";

pub struct Artifacts {
    pub spirv_filename: String,
    pub reflection: Vec<u8>,
    pub abi: Vec<u8>,
}

pub struct Compiler {
    executable: PathBuf,
    reflector: PathBuf,
}

impl Compiler {
    pub fn new(manifest: &Path, output: &Path) -> Self {
        let source = env::var_os("SLANG_SOURCE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| manifest.join("../..").join("external/slang"));
        let build = env::var_os("SLANG_BUILD_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| source.join("build"));
        let executable = build.join(CONFIGURATION).join("bin").join("slangc");

        assert!(
            source.join("CMakeLists.txt").is_file(),
            "Slang source checkout not found at {}; set SLANG_SOURCE_DIR",
            source.display()
        );
        fs::create_dir_all(&build)
            .unwrap_or_else(|error| panic!("create Slang build directory: {error}"));
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(build.join("shrimply.lock"))
            .unwrap_or_else(|error| panic!("open Slang build lock: {error}"));
        lock.lock()
            .unwrap_or_else(|error| panic!("lock Slang build: {error}"));
        if !executable.is_file() || !has_glslang(&build) {
            configure(&source, &build);
            run(
                Command::new("cmake").arg("--build").arg(&build).args([
                    "--config",
                    CONFIGURATION,
                    "--target",
                    "slangc",
                    "slang-glslang",
                ]),
                "build Slang compiler and SPIR-V optimizer",
            );
        }
        drop(lock);
        let reflector = output.join("slang-reflect");
        build_reflector(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("reflect.cpp"),
            &source,
            &build,
            &reflector,
        );
        Self {
            executable,
            reflector,
        }
    }

    pub fn compile(&self, directory: &Path, source: &Path, output: &Path) -> Artifacts {
        let module = source
            .file_stem()
            .and_then(|name| name.to_str())
            .expect("Slang module filename must be UTF-8");
        let spirv_filename = format!("{module}.spv");
        let spirv = output.join(&spirv_filename);
        let reflection = output.join(format!("{module}.reflection.json"));
        let abi = output.join(format!("{module}.abi"));
        run(
            Command::new(&self.executable)
                .arg(source)
                .arg("-I")
                .arg(directory)
                .arg("-I")
                .arg(directory.join("modules"))
                .args([
                    "-target",
                    "spirv",
                    "-profile",
                    "glsl_460+spirv_1_5",
                    "-capability",
                    "spvGroupNonUniform+spvGroupNonUniformBallot",
                    "-matrix-layout-column-major",
                    "-emit-spirv-directly",
                    "-O2",
                    "-reflection-json",
                ])
                .arg(&reflection)
                .arg("-o")
                .arg(&spirv),
            &format!("compile Slang module `{module}` and reflect its bindings"),
        );
        run(
            Command::new(&self.reflector)
                .arg(directory)
                .arg(module)
                .arg(&abi),
            &format!("reflect Slang ABI declarations in module `{module}`"),
        );
        Artifacts {
            spirv_filename,
            reflection: fs::read(&reflection)
                .unwrap_or_else(|error| panic!("read Slang reflection for {module}: {error}")),
            abi: fs::read(&abi)
                .unwrap_or_else(|error| panic!("read Slang ABI for {module}: {error}")),
        }
    }
}

fn build_reflector(source: &Path, slang: &Path, build: &Path, output: &Path) {
    let header = slang.join("include/slang.h");
    if is_current(output, &[source, &header]) {
        return;
    }
    let library = build.join(CONFIGURATION).join("lib");
    let compiler = env::var_os("CXX").unwrap_or_else(|| "c++".into());
    run(
        Command::new(compiler)
            .args(["-std=c++17", "-O2"])
            .arg(source)
            .arg("-I")
            .arg(slang.join("include"))
            .arg("-L")
            .arg(&library)
            .arg(format!("-Wl,-rpath,{}", library.display()))
            .arg("-lslang")
            .arg("-o")
            .arg(output),
        "build Slang ABI reflector",
    );
}

fn is_current(output: &Path, inputs: &[&Path]) -> bool {
    let Ok(output_modified) = output.metadata().and_then(|metadata| metadata.modified()) else {
        return false;
    };
    inputs.iter().all(|input| {
        input
            .metadata()
            .and_then(|metadata| metadata.modified())
            .is_ok_and(|modified| modified <= output_modified)
    })
}

pub fn shader_sources(directory: &Path) -> Vec<PathBuf> {
    let mut sources: Vec<_> = directory
        .read_dir()
        .unwrap_or_else(|error| panic!("read shader directory {}: {error}", directory.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == EXTENSION)
        })
        .collect();
    sources.sort();
    assert!(
        !sources.is_empty(),
        "no .slang modules found in {}",
        directory.display()
    );
    sources
}

fn configure(source: &Path, build: &Path) {
    run(
        Command::new("cmake")
            .arg("-S")
            .arg(source)
            .arg("-B")
            .arg(build)
            .args([
                "-G",
                "Ninja Multi-Config",
                "-DSLANG_ENABLE_SLANGC=ON",
                "-DSLANG_ENABLE_SLANG_RHI=OFF",
                "-DSLANG_ENABLE_GFX=OFF",
                "-DSLANG_ENABLE_TESTS=OFF",
                "-DSLANG_ENABLE_EXAMPLES=OFF",
                "-DSLANG_ENABLE_SLANGD=OFF",
                "-DSLANG_ENABLE_SLANGI=OFF",
                "-DSLANG_ENABLE_SLANGRT=OFF",
                "-DSLANG_ENABLE_SLANG_GLSLANG=ON",
                "-DSLANG_ENABLE_REPLAYER=OFF",
                "-DSLANG_SLANG_LLVM_FLAVOR=DISABLE",
                "-DSLANG_ENABLE_DXIL=OFF",
            ]),
        "configure Slang compiler",
    );
}

fn has_glslang(build: &Path) -> bool {
    build
        .join(CONFIGURATION)
        .join("lib")
        .read_dir()
        .is_ok_and(|entries| {
            entries.filter_map(Result::ok).any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(GLSLANG_PREFIX)
            })
        })
}

fn run(command: &mut Command, action: &str) {
    if let Ok(mut terminal) = OpenOptions::new().write(true).open("/dev/tty") {
        writeln!(terminal, "[slang] {action}").expect("write build progress");
        let stderr = terminal.try_clone().expect("clone build terminal");
        let status = command
            .stdout(Stdio::from(terminal))
            .stderr(Stdio::from(stderr))
            .status()
            .unwrap_or_else(|error| panic!("failed to {action}: {error}"));
        assert!(status.success(), "failed to {action}: {status}");
        return;
    }

    println!("[shrimply] {action}");
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("failed to {action}: {error}"));
    let stdout = child.stdout.take().expect("capture command stdout");
    let stderr = child.stderr.take().expect("capture command stderr");
    let stdout = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            println!("{}", line.expect("read command stdout"));
        }
    });
    let stderr = thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            eprintln!("{}", line.expect("read command stderr"));
        }
    });
    let status = child
        .wait()
        .unwrap_or_else(|error| panic!("failed to wait while trying to {action}: {error}"));
    stdout.join().expect("relay command stdout");
    stderr.join().expect("relay command stderr");
    assert!(status.success(), "failed to {action}: {status}");
}
