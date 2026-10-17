pub mod auth;
// The deep-link swap point: this module is deleted when the cogniclone:// URL
// scheme lands (browser-login, task 8.2). Nothing else in network/ changes.
pub mod loopback;
pub mod jobs;
pub mod pipelines;
pub mod quota;
pub mod reports;
pub mod sse;
pub mod upload;
