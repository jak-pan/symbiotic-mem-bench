pub mod membench {
    pub mod dashboard {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/membench.dashboard.v1.rs"));
        }
    }
}
