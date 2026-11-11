#![cfg(target_os = "linux")]
use skilltape_runner::{ProcessAdapter, ProcessRequest, TokioProcessAdapter};
use std::fs;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn external_executable_is_available_without_its_siblings_and_cannot_be_written() {
    let temp = tempfile::tempdir().unwrap();
    let program = temp.path().join("local-tool");
    fs::copy("/bin/sh", &program).unwrap();
    let secret = temp.path().join("private-neighbor");
    fs::write(&secret, b"synthetic private neighbor").unwrap();
    let workspace = temp.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    let output=TokioProcessAdapter.run(ProcessRequest {
        program:program.to_string_lossy().into_owned(),
        args:vec!["-c".into(),"test ! -e \"$1\" && ! (printf changed > /.skilltape-executable) 2>/dev/null && printf isolated".into(),"probe".into(),secret.to_string_lossy().into_owned()],
        cwd:workspace,timeout:Duration::from_secs(5),max_output_bytes:1024,
    },CancellationToken::new()).await.unwrap();
    assert_eq!(output.exit_code, Some(0), "{:?}", output.stderr);
    assert_eq!(output.stdout, b"isolated");
    assert_eq!(fs::read(&secret).unwrap(), b"synthetic private neighbor");
    assert_eq!(fs::read(&program).unwrap(), fs::read("/bin/sh").unwrap());
}
