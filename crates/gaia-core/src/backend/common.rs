use crate::command_spec::CommandSpec;

pub(crate) fn add_container_hardening_args(args: &mut Vec<String>, user: &str) {
    args.extend([
        "--security-opt".to_owned(),
        "no-new-privileges:true".to_owned(),
        "--cap-drop".to_owned(),
        "ALL".to_owned(),
        "--read-only".to_owned(),
        "--tmpfs".to_owned(),
        "/tmp".to_owned(),
        "--user".to_owned(),
        user.to_owned(),
    ]);
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DockerRunProfile<'a> {
    pub container_name: &'a str,
    pub user: &'a str,
    pub host_port: u16,
    pub container_port: u16,
    pub detach: bool,
    pub gpu_all: bool,
    pub runtime: Option<&'a str>,
}

pub(crate) fn base_docker_run_args(profile: DockerRunProfile<'_>) -> Vec<String> {
    let mut args = vec![
        "run".to_owned(),
        "--rm".to_owned(),
        "--name".to_owned(),
        profile.container_name.to_owned(),
    ];

    if let Some(runtime) = profile.runtime {
        args.extend(["--runtime".to_owned(), runtime.to_owned()]);
    }

    if profile.gpu_all {
        args.extend(["--gpus".to_owned(), "all".to_owned()]);
    }

    args.extend([
        "-p".to_owned(),
        format!("{}:{}", profile.host_port, profile.container_port),
    ]);
    add_container_hardening_args(&mut args, profile.user);

    if profile.detach {
        args.push("-d".to_owned());
    }

    args
}

pub(crate) fn add_optional_hf_token_arg(args: &mut Vec<String>, hf_token: Option<&str>) {
    if hf_token.is_some() {
        args.extend(["-e".to_owned(), "HF_TOKEN".to_owned()]);
    }
}

pub(crate) fn apply_optional_hf_token_env(
    mut command: CommandSpec,
    hf_token: Option<&str>,
) -> CommandSpec {
    if let Some(token) = hf_token {
        command = command.env("HF_TOKEN", token);
    }
    command
}
