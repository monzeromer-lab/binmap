//! One sweep at a time per project.
//!
//! Cargo takes its own lock on a build directory, so two sweeps never compile
//! into the same place simultaneously. What cargo does not protect is what we
//! do *after* measuring: `Sweep::reclaim` deletes a per-configuration build
//! directory once its artifact has been copied out. Two sweeps over one
//! project will happily delete directories out from under each other, and the
//! result is not a crash — it is a run that reports "0 of 96 passed" because
//! its builds kept vanishing. That happened here, and the numbers looked like
//! a real regression until the two runs were separated.
//!
//! So a sweep holds an exclusive advisory lock on the project for its
//! duration, and a second one fails immediately with a message that says what
//! holds it. Refusing is better than the silent wrong answer, and it is what
//! cargo itself does.

use binmap_core::error::{Error, Result};
use fs4::fs_std::FileExt;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

/// An exclusive claim on one project's binmap directory.
///
/// The lock is released when this is dropped, including on panic and on
/// process death — an advisory file lock is held by the open descriptor, not
/// by anything we have to remember to clean up. A stale lock file left behind
/// by a killed process therefore does not block the next run.
#[derive(Debug)]
pub struct ProjectLock {
    file: File,
    path: PathBuf,
}

impl ProjectLock {
    /// Claim `directory`, or say who has it.
    ///
    /// Does not block. A sweep can run for an hour, and silently waiting an
    /// hour is indistinguishable from a hang.
    pub fn acquire(directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory).map_err(|error| {
            Error::Other(format!("could not create {}: {error}", directory.display()))
        })?;
        let path = directory.join(".binmap-lock");

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| Error::Other(format!("could not open {}: {error}", path.display())))?;

        match file.try_lock_exclusive() {
            Ok(true) => {}
            Ok(false) | Err(_) => {
                // Whatever the holder wrote is a hint, not a fact: it may be
                // a process that has since died in a way that left the file.
                // The lock itself is the authority, so the message is phrased
                // as what we observed rather than as a diagnosis.
                let holder = std::fs::read_to_string(&path).unwrap_or_default();
                let holder = holder.trim();
                let who = if holder.is_empty() {
                    "another binmap run".to_string()
                } else {
                    format!("another binmap run ({holder})")
                };
                return Err(Error::Other(format!(
                    "{who} is already working in {}. Two sweeps over one project delete each \
                     other's build directories, so this one is stopping rather than reporting \
                     numbers it cannot trust. Wait for that run to finish, or sweep a copy of \
                     the project.",
                    directory.display()
                )));
            }
        }

        // Record who holds it, for the message the *next* caller gets.
        let _ = std::fs::write(&path, format!("pid {} since {}", std::process::id(), now()));

        Ok(Self { file, path })
    }

    /// Where the lock lives, for tests and for messages.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ProjectLock {
    fn drop(&mut self) {
        // Blank the hint before releasing, so a stale pid is never read as a
        // live holder. The unlock is what matters; both are best-effort.
        let _ = std::fs::write(&self.path, "");
        let _ = FileExt::unlock(&self.file);
    }
}

fn now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| format!("epoch+{}s", elapsed.as_secs()))
        .unwrap_or_else(|_| "an unknown time".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_claim_on_the_same_directory_is_refused() {
        let scratch = tempfile::tempdir().expect("tempdir");
        let first = ProjectLock::acquire(scratch.path()).expect("first claim");

        let error = ProjectLock::acquire(scratch.path())
            .expect_err("the second claim must be refused, not granted");
        let message = error.to_string();
        assert!(
            message.contains("already working"),
            "the refusal should say the project is taken, got: {message}"
        );
        assert!(
            message.contains("sweep a copy"),
            "the refusal should say what to do about it, got: {message}"
        );

        drop(first);
    }

    #[test]
    fn releasing_lets_the_next_run_in() {
        let scratch = tempfile::tempdir().expect("tempdir");
        let first = ProjectLock::acquire(scratch.path()).expect("first claim");
        let path = first.path().to_path_buf();
        drop(first);

        let second = ProjectLock::acquire(scratch.path())
            .expect("a released lock must not block the next run");
        assert_eq!(second.path(), path, "the same lock file is reused");
    }

    #[test]
    fn two_different_projects_do_not_contend() {
        let one = tempfile::tempdir().expect("tempdir");
        let two = tempfile::tempdir().expect("tempdir");
        let _first = ProjectLock::acquire(one.path()).expect("first project");
        let _second = ProjectLock::acquire(two.path())
            .expect("a lock is per project, so a second project must be free");
    }

    #[test]
    fn a_stale_lock_file_with_no_holder_does_not_block() {
        // A process killed mid-sweep leaves the file behind. The OS drops the
        // advisory lock with the descriptor, so the file alone must not be
        // treated as a claim.
        let scratch = tempfile::tempdir().expect("tempdir");
        std::fs::write(scratch.path().join(".binmap-lock"), "pid 999999 since epoch+1s")
            .expect("write a stale lock file");

        ProjectLock::acquire(scratch.path())
            .expect("a lock file nobody holds must not block a new run");
    }
}
