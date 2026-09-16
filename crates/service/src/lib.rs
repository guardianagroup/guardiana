//! Daemon: Windows SCM and systemd; system DNS change and restore; watchdog
//! (brief §2, §4). Week 1 ships only the read-only part: finding the
//! resolvers the system uses today, which `guardiana observe` forwards to.

pub mod daemon;
pub mod home;
pub mod sysdns;
