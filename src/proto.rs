//! Bench-owned contract schemas, generated from `proto/membench/**` by
//! `build.rs` (prost). See `proto/CONTRACTS.md` for ownership and evolution
//! rules (additive-only; `sha256` lowercase-hex digests; caps_hash as the
//! comparability partition key).

pub mod trace {
    pub mod v1 {
        include!(concat!(env!("OUT_DIR"), "/membench.trace.v1.rs"));
    }
}

pub mod manifest {
    pub mod v1 {
        include!(concat!(env!("OUT_DIR"), "/membench.manifest.v1.rs"));
    }
}

pub mod scorecard {
    pub mod v1 {
        include!(concat!(env!("OUT_DIR"), "/membench.scorecard.v1.rs"));
    }
}

#[cfg(test)]
mod tests {
    use prost::Message;

    #[test]
    fn step_envelope_round_trips_with_links_and_timing() {
        use super::trace::v1 as trace;

        let envelope = trace::StepEnvelope {
            schema_version: 1,
            step_id: "step-1".into(),
            run_id: "run-1".into(),
            family: trace::TraceFamily::Memory as i32,
            kind: "capture".into(),
            links: vec![trace::Link {
                r#ref: "step-0".into(),
                rel: trace::LinkRelation::FollowsFrom as i32,
            }],
            timing: Some(trace::Timing {
                queued_ms: Some(5),
                budget_wait_ms: Some(0),
                cooldown_wait_ms: Some(0),
                exec_ms: Some(42),
            }),
            ..Default::default()
        };

        let bytes = envelope.encode_to_vec();
        let decoded = trace::StepEnvelope::decode(bytes.as_slice()).expect("decode");

        assert_eq!(decoded.step_id, "step-1");
        assert_eq!(decoded.family, trace::TraceFamily::Memory as i32);
        assert_eq!(decoded.links.len(), 1);
        assert_eq!(decoded.timing.and_then(|t| t.exec_ms), Some(42));
    }

    #[test]
    fn scorecard_round_trips_with_provenance() {
        use super::scorecard::v1 as scorecard;

        let card = scorecard::Scorecard {
            schema_version: 1,
            ..Default::default()
        };
        let bytes = card.encode_to_vec();
        let decoded = scorecard::Scorecard::decode(bytes.as_slice()).expect("decode");
        assert_eq!(decoded.schema_version, 1);
    }
}
