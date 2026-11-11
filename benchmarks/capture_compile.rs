use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::time::Instant;

use serde_json::json;
use skilltape_compiler::{
    CompileRequest, CompileTarget, Compiler, DeterministicCompiler, TapeSession,
};
use skilltape_tape::{
    EventSource, RedactionState, TapeEvent, TapeEventKind, TapeManifest, TapeStore, TAPE_SCHEMA_V1,
};
use tempfile::TempDir;

const TAPE_EVENT_COUNT: u64 = 10_000;
const WORKFLOW_STEP_COUNT: usize = 100;
const LARGE_LOG_BYTES: u64 = 1_000_000_000;

fn main() {
    benchmark_tape_storage();
    benchmark_workflow_compile();
    if std::env::var_os("SKILLTAPE_BENCHMARK_LARGE").is_some() {
        benchmark_large_log();
    } else {
        println!(
            "benchmark=1gb-log status=skipped bytes={LARGE_LOG_BYTES} reason=opt-in with SKILLTAPE_BENCHMARK_LARGE=1"
        );
    }
}

fn benchmark_tape_storage() {
    let temp = TempDir::new().expect("benchmark temp directory");
    let root = temp.path().join("tape");
    fs::create_dir(&root).expect("create tape root");
    let manifest = TapeManifest {
        schema: TAPE_SCHEMA_V1.to_owned(),
        id: "benchmark".to_owned(),
        started_at_ms: 1,
        finished_at_ms: Some(TAPE_EVENT_COUNT + 1),
        platform: std::env::consts::OS.to_owned(),
        workspace_root: "benchmark".to_owned(),
        event_count: TAPE_EVENT_COUNT,
    };

    let started = Instant::now();
    serde_json::to_writer(
        File::create(root.join("manifest.json")).expect("create manifest"),
        &manifest,
    )
    .expect("write manifest");
    let events_path = File::create(root.join("events.jsonl")).expect("create events");
    let mut events_file = BufWriter::new(events_path);
    for sequence in 0..TAPE_EVENT_COUNT {
        serde_json::to_writer(
            &mut events_file,
            &TapeEvent {
                sequence,
                occurred_at_ms: sequence,
                kind: TapeEventKind::CaptureWarning,
                source: EventSource::System,
                payload: json!({"benchmark_sequence": sequence}),
                redaction: RedactionState::Unredacted,
            },
        )
        .expect("write benchmark event");
        events_file.write_all(b"\n").expect("terminate event");
    }
    events_file.flush().expect("flush events");
    let store = TapeStore::open(&root).expect("open benchmark tape");
    let read_count = store.read_events().expect("read benchmark events").count();
    let elapsed = started.elapsed();
    println!(
        "benchmark=tape-storage events={TAPE_EVENT_COUNT} read_events={read_count} mode=streamed-jsonl elapsed_ms={}",
        elapsed.as_millis()
    );
}

fn benchmark_workflow_compile() {
    let events = (0..WORKFLOW_STEP_COUNT)
        .flat_map(|step| {
            let start = (step * 2) as u64;
            [
                TapeEvent {
                    sequence: start,
                    occurred_at_ms: start,
                    kind: TapeEventKind::TerminalCommand,
                    source: EventSource::Shell,
                    payload: json!({
                        "phase": "started",
                        "command": "/bin/echo",
                        "args": [],
                        "cwd": "/workspace"
                    }),
                    redaction: RedactionState::Unredacted,
                },
                TapeEvent {
                    sequence: start + 1,
                    occurred_at_ms: start + 1,
                    kind: TapeEventKind::TerminalCommand,
                    source: EventSource::Shell,
                    payload: json!({
                        "phase": "output",
                        "text": "benchmark\n",
                        "truncated": false,
                        "redactions": []
                    }),
                    redaction: RedactionState::Unredacted,
                },
            ]
        })
        .collect::<Vec<_>>();
    let session = TapeSession::new(events).expect("benchmark tape session");
    let request = CompileRequest::new(
        session,
        "benchmark-workflow",
        CompileTarget::new("generic-agent-skill", "0.1.0").expect("target"),
    )
    .expect("compile request");

    let started = Instant::now();
    let output = DeterministicCompiler
        .compile(request)
        .expect("compile benchmark workflow");
    let elapsed = started.elapsed();
    assert_eq!(output.workflow.steps.len(), WORKFLOW_STEP_COUNT);
    println!(
        "benchmark=workflow-compile steps={WORKFLOW_STEP_COUNT} elapsed_ms={}",
        elapsed.as_millis()
    );
}

fn benchmark_large_log() {
    let temp = TempDir::new().expect("benchmark temp directory");
    let path = temp.path().join("large.log");
    let started = Instant::now();
    let file = File::create(&path).expect("large log");
    file.set_len(LARGE_LOG_BYTES).expect("sparse large log");
    file.sync_all().expect("sync large log");
    drop(file);

    let mut reader = File::open(&path).expect("open large log");
    let mut buffer = [0_u8; 64 * 1024];
    let mut bytes_read = 0_u64;
    loop {
        let read = reader.read(&mut buffer).expect("read large log");
        if read == 0 {
            break;
        }
        bytes_read = bytes_read.saturating_add(read as u64);
    }
    let elapsed = started.elapsed();
    assert_eq!(bytes_read, LARGE_LOG_BYTES);
    let metadata_bytes = fs::metadata(&path).expect("large log metadata").len();
    println!(
        "benchmark=1gb-log bytes={bytes_read} metadata_bytes={metadata_bytes} elapsed_ms={}",
        elapsed.as_millis()
    );
}
