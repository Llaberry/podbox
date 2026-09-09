//! What the payload will be: the argv, the environment, the working directory,
//! and the banner that says what podbox is and is not doing.
//!
//! ⭐ **docker's rules for combining an image's `Entrypoint` and `Cmd` with the
//! caller's arguments are not obvious and are not podbox's to invent.** They are
//! implemented here as data and asserted, because getting them wrong runs
//! something the caller did not ask for and the failure looks like the image's.

use crate::Fds;

/// How the program will be looked for inside the new root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Program {
    /// The argument had a `/` in it, so it is a path and `PATH` is not consulted.
    Absolute(String),
    /// A bare name, to be looked for along `PATH`.
    OnPath(String),
}

pub struct Plan {
    pub argv: Vec<String>,
    pub env: Vec<String>,
    pub working_dir: String,
    pub fds: Fds,
    pub banner: String,
    /// The `PATH` the program is looked for along, already split.
    pub path_dirs: Vec<String>,
}

impl Plan {
    /// The image's `Entrypoint` and `Cmd`, combined with what the caller wrote.
    ///
    /// ⛔ **docker's rules, and each one has a reason a caller relies on:**
    ///
    /// | image has | caller wrote | what runs |
    /// | --- | --- | --- |
    /// | `Entrypoint` | nothing | `Entrypoint` + `Cmd` |
    /// | `Entrypoint` | `args` | `Entrypoint` + `args`, and `Cmd` is DROPPED |
    /// | no `Entrypoint` | nothing | `Cmd` |
    /// | no `Entrypoint` | `args` | `args`, and `Cmd` is DROPPED |
    ///
    /// ⚠ The row that surprises people is the second: an image's `Cmd` is the
    /// **default arguments to its entrypoint**, so a caller supplying arguments
    /// replaces them. Appending both would pass the image's defaults *and* the
    /// caller's, which is how `docker run img --help` ends up running the
    /// default command with `--help` bolted on.
    pub fn argv_for(
        entrypoint: Option<&[String]>,
        cmd: Option<&[String]>,
        caller: &[String],
        override_entrypoint: Option<&str>,
    ) -> Vec<String> {
        // ⚠ `--entrypoint` REPLACES the image's, and then the image's `Cmd` is
        // dropped too: docker's rule, because the default arguments belonged to
        // the entrypoint that is no longer running.
        if let Some(e) = override_entrypoint {
            let mut v = vec![e.to_string()];
            v.extend_from_slice(caller);
            return v;
        }
        let mut v: Vec<String> = entrypoint.map(<[String]>::to_vec).unwrap_or_default();
        if caller.is_empty() {
            v.extend(cmd.map(<[String]>::to_vec).unwrap_or_default());
        } else {
            v.extend_from_slice(caller);
        }
        v
    }

    /// The image's `Env`, then the caller's, so the caller wins.
    ///
    /// ⛔ **A later assignment of the same name wins and the earlier one is
    /// removed rather than shadowed.** `execve` takes an array, not a map, and a
    /// duplicate name in it is resolved by whichever the libc's `getenv` finds
    /// first, which is not the same on glibc and musl. podbox decides here so
    /// the answer does not depend on the payload's libc.
    pub fn env_for(image: &[String], caller: &[String]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for e in image.iter().chain(caller.iter()) {
            let name = e.split('=').next().unwrap_or("");
            out.retain(|existing: &String| existing.split('=').next().unwrap_or("") != name);
            out.push(e.clone());
        }
        out
    }

    /// Where `PATH` comes from, with a default that is not invented.
    ///
    /// ⚠ The fallback is the one POSIX names and every base image sets anyway.
    /// It is here so a payload whose image config carries no `Env` still finds
    /// `/bin/sh`, and it is a **documented default** rather than a silent one:
    /// the banner prints the `PATH` that was used.
    pub const DEFAULT_PATH: &'static str =
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

    pub fn path_from(env: &[String]) -> Vec<String> {
        let raw = env
            .iter()
            .rev()
            .find_map(|e| e.strip_prefix("PATH="))
            .unwrap_or(Self::DEFAULT_PATH);
        raw.split(':')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// How the first argument will be looked for.
    pub fn program(&self) -> Option<Program> {
        let first = self.argv.first()?;
        if first.contains('/') {
            Some(Program::Absolute(first.clone()))
        } else {
            Some(Program::OnPath(first.clone()))
        }
    }

    /// Every path `execve` will be tried against, in order.
    ///
    /// ⛔ Built before the fork and tried after it. T-0502: resolving in the
    /// parent names the outer tree, and the outer tree is gone.
    pub fn program_candidates(&self) -> Vec<String> {
        match self.program() {
            None => Vec::new(),
            Some(Program::Absolute(p)) => vec![p],
            Some(Program::OnPath(name)) => self
                .path_dirs
                .iter()
                .map(|d| format!("{}/{name}", d.trim_end_matches('/')))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn dockers_entrypoint_and_cmd_rules_are_the_ones_implemented() {
        let ep = v(&["/ep"]);
        let cmd = v(&["a", "b"]);

        // Entrypoint + nothing -> Entrypoint + Cmd
        assert_eq!(
            Plan::argv_for(Some(&ep), Some(&cmd), &[], None),
            v(&["/ep", "a", "b"])
        );
        // ⛔ Entrypoint + args -> Entrypoint + args, and Cmd is DROPPED. An
        // image's Cmd is the default ARGUMENTS to its entrypoint, so a caller
        // supplying arguments replaces them rather than appending to them.
        assert_eq!(
            Plan::argv_for(Some(&ep), Some(&cmd), &v(&["z"]), None),
            v(&["/ep", "z"])
        );
        // no Entrypoint + nothing -> Cmd
        assert_eq!(Plan::argv_for(None, Some(&cmd), &[], None), v(&["a", "b"]));
        // no Entrypoint + args -> args
        assert_eq!(
            Plan::argv_for(None, Some(&cmd), &v(&["z"]), None),
            v(&["z"])
        );
        // --entrypoint replaces, and drops Cmd with it
        assert_eq!(
            Plan::argv_for(Some(&ep), Some(&cmd), &[], Some("/other")),
            v(&["/other"])
        );
        assert_eq!(
            Plan::argv_for(Some(&ep), Some(&cmd), &v(&["z"]), Some("/other")),
            v(&["/other", "z"])
        );
    }

    #[test]
    fn an_empty_entrypoint_array_is_not_the_same_as_an_absent_one() {
        // ⛔ An image that sets `"Entrypoint": []` has explicitly CLEARED its
        // parent's, and treating that as unset would run the parent's.
        let empty: Vec<String> = Vec::new();
        let cmd = v(&["c"]);
        assert_eq!(
            Plan::argv_for(Some(&empty), Some(&cmd), &[], None),
            v(&["c"])
        );
    }

    #[test]
    fn the_caller_wins_an_environment_name_and_the_image_copy_is_removed() {
        // ⛔ Removed, not shadowed: `execve` takes an array, and which of two
        // entries with one name `getenv` finds is not the same on glibc and
        // musl. podbox decides here so the payload's libc cannot.
        let out = Plan::env_for(&v(&["A=1", "B=2"]), &v(&["A=9"]));
        assert_eq!(out, v(&["B=2", "A=9"]));
        assert_eq!(out.iter().filter(|e| e.starts_with("A=")).count(), 1);
    }

    #[test]
    fn path_comes_from_the_environment_and_falls_back_to_a_stated_default() {
        assert_eq!(Plan::path_from(&v(&["PATH=/x:/y"])), v(&["/x", "/y"]),);
        // ⚠ The LAST assignment wins, matching env_for's own rule.
        assert_eq!(Plan::path_from(&v(&["PATH=/a", "PATH=/b"])), v(&["/b"]));
        assert!(Plan::path_from(&[]).contains(&"/bin".to_string()));
    }

    #[test]
    fn a_name_with_a_slash_is_a_path_and_never_searched_on_path() {
        let mut p = Plan {
            argv: v(&["/bin/echo", "hi"]),
            env: Vec::new(),
            working_dir: String::new(),
            fds: Fds::default(),
            banner: String::new(),
            path_dirs: v(&["/usr/bin", "/bin"]),
        };
        assert_eq!(p.program_candidates(), v(&["/bin/echo"]));

        p.argv = v(&["echo"]);
        assert_eq!(p.program_candidates(), v(&["/usr/bin/echo", "/bin/echo"]));

        // ⚠ A relative path with a slash is still a path, not a PATH lookup:
        // `./x` means the working directory's `x`.
        p.argv = v(&["./x"]);
        assert_eq!(p.program_candidates(), v(&["./x"]));
    }
}
