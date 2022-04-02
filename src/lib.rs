//! Encrypted backups.
//!
//! Obnam is a backup program that encrypts the backups. This crate
//! provides access to all the functionality of Obnam as a library.

#![deny(missing_docs)]

pub mod accumulated_time;
pub mod backup_progress;
pub mod backup_reason;
pub mod backup_run;
pub mod checksummer;
pub mod chunk;
pub mod chunker;
pub mod chunkid;
pub mod chunkmeta;
pub mod cipher;
pub mod client;
pub mod cmd;
pub mod config;
pub mod db;
pub mod dbgen;
pub mod engine;
pub mod error;
pub mod fsentry;
pub mod fsiter;
pub mod generation;
pub mod genlist;
pub mod genmeta;
pub mod index;
pub mod indexedstore;
pub mod passwords;
pub mod performance;
pub mod policy;
pub mod schema;
pub mod server;
pub mod store;
pub mod workqueue;
