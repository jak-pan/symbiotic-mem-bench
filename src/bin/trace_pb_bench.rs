use prost::Message;
use serde_json::{Map, Number, Value};
use std::{env, fs, time::Instant};
use symbiotic_mem_bench::dashboard_proto::membench::dashboard::v1::{
    TraceEventRow, TracesResponse,
};

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs[xs.len() / 2]
}

fn opt_f64(v: Option<f64>) -> Value {
    v.and_then(Number::from_f64)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn materialize_trace_events(msg: &TracesResponse) -> usize {
    let Some(stream) = &msg.trace_events else {
        return 0;
    };
    for row in &stream.rows {
        let TraceEventRow {
            timestamp,
            kind,
            operation,
            lane,
            event,
            status,
            attempt,
            duration_ms,
            wait_ms,
            run_ms,
            total_ms,
            item_count,
            item_unit,
            source,
            error,
        } = row;
        let mut out = Map::new();
        out.insert("timestamp".into(), Value::String(timestamp.clone()));
        out.insert("kind".into(), Value::String(kind.clone()));
        out.insert("operation".into(), Value::String(operation.clone()));
        out.insert("lane".into(), Value::String(lane.clone()));
        out.insert("event".into(), Value::String(event.clone()));
        out.insert("status".into(), Value::String(status.clone()));
        out.insert("attempt".into(), Value::Number(Number::from(*attempt)));
        out.insert("duration_ms".into(), opt_f64(*duration_ms));
        out.insert("wait_ms".into(), opt_f64(*wait_ms));
        out.insert("run_ms".into(), opt_f64(*run_ms));
        out.insert("total_ms".into(), opt_f64(*total_ms));
        out.insert(
            "item_count".into(),
            Value::Number(Number::from(*item_count)),
        );
        out.insert("item_unit".into(), Value::String(item_unit.clone()));
        out.insert("source".into(), Value::String(source.clone()));
        out.insert(
            "error".into(),
            error.clone().map(Value::String).unwrap_or(Value::Null),
        );
        std::hint::black_box(out);
    }
    stream.rows.len()
}

fn main() {
    let path = env::args()
        .nth(1)
        .expect("usage: trace_pb_bench <traces.pb>");
    let loops: usize = env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    let bytes = fs::read(path).expect("read pb");

    let warm = TracesResponse::decode(bytes.as_slice()).expect("warm decode");
    println!("bytes={}", bytes.len());
    println!(
        "memory_trace_total={}",
        warm.memory_traces.as_ref().map(|m| m.total).unwrap_or(0)
    );
    println!(
        "memory_trace_truncated={}",
        warm.memory_traces
            .as_ref()
            .map(|m| m.truncated)
            .unwrap_or(false)
    );
    println!(
        "trace_event_rows={}",
        warm.trace_events
            .as_ref()
            .map(|m| m.rows.len())
            .unwrap_or(0)
    );
    println!(
        "trace_event_total={}",
        warm.trace_events.as_ref().map(|m| m.total).unwrap_or(0)
    );
    println!("memory_stage_timing={}", warm.memory_stage_timing.len());
    println!("queue_timing={}", warm.queue_timing.len());
    println!("has_workflow_queue={}", warm.workflow_queue.is_some());

    let mut decode_ms = Vec::with_capacity(loops);
    let mut encode_ms = Vec::with_capacity(loops);
    let mut materialize_ms = Vec::with_capacity(loops);
    let mut encoded_len = 0usize;
    let mut materialized_rows = 0usize;

    for _ in 0..loops {
        let t0 = Instant::now();
        let msg = TracesResponse::decode(bytes.as_slice()).expect("decode");
        decode_ms.push(t0.elapsed().as_secs_f64() * 1000.0);

        let t1 = Instant::now();
        let out = msg.encode_to_vec();
        encode_ms.push(t1.elapsed().as_secs_f64() * 1000.0);
        encoded_len = out.len();

        let t2 = Instant::now();
        materialized_rows += materialize_trace_events(&msg);
        materialize_ms.push(t2.elapsed().as_secs_f64() * 1000.0);
    }

    println!("encoded_len={}", encoded_len);
    println!("loops={}", loops);
    println!("decode_ms_median={:.3}", median(decode_ms.clone()));
    println!(
        "decode_ms_min={:.3}",
        decode_ms.iter().copied().fold(f64::INFINITY, f64::min)
    );
    println!("encode_ms_median={:.3}", median(encode_ms.clone()));
    println!(
        "encode_ms_min={:.3}",
        encode_ms.iter().copied().fold(f64::INFINITY, f64::min)
    );
    println!(
        "materialize_trace_events_ms_median={:.3}",
        median(materialize_ms.clone())
    );
    println!(
        "materialize_trace_events_ms_min={:.3}",
        materialize_ms.iter().copied().fold(f64::INFINITY, f64::min)
    );
    println!("materialized_trace_events_total={}", materialized_rows);
}
