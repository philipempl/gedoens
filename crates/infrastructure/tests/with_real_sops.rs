//! Spielt die Anwendungsfälle gegen das echte `sops` und das echte `git` durch.
//! Fehlt eines der beiden Programme, wird der Test übersprungen.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use gedoens_application::ports::KeyRing;
use gedoens_application::views::{Choice, Issue, Step};
use gedoens_application::Gedoens;
use gedoens_infrastructure::{AgeKeyRing, FsWorkspace, GitCli, SopsCli, SopsConfigFile};

fn tool(name: &str) -> Option<std::path::PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).map(|dir| dir.join(name)).find(|p| p.is_file())
}

fn run(program: &Path, folder: &Path, args: &[&str]) {
    let output = Command::new(program).current_dir(folder).args(args).output().unwrap();
    assert!(output.status.success(), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
}

fn key(name: &str) -> Vec<Step> {
    vec![Step::Key(name.to_owned())]
}

#[test]
fn the_whole_story() {
    let (Some(sops), Some(git)) = (tool("sops"), tool("git")) else {
        eprintln!("sops oder git fehlt, Test übersprungen");
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path();

    // Zwei Menschen mit je einem Key. Wir sind der erste.
    let my_keys = home.path().join("me/keys.txt");
    let me = AgeKeyRing::at(my_keys.clone()).generate().unwrap();
    let anna = AgeKeyRing::at(home.path().join("anna/keys.txt")).generate().unwrap();
    std::env::set_var("SOPS_AGE_KEY_FILE", &my_keys);

    fs::create_dir_all(root.join("secrets/prod")).unwrap();
    fs::create_dir_all(root.join("secrets/dev")).unwrap();
    fs::write(
        root.join(".sops.yaml"),
        format!("creation_rules:\n  - path_regex: secrets/prod/.*\n    age:\n      - {}\n  - path_regex: secrets/dev/.*\n    age: {}\n", me.as_str(), me.as_str()),
    )
    .unwrap();
    fs::write(root.join("secrets/prod/api.yaml"), "db:\n  user: api\n  port: 5432\ntoken: abc\n").unwrap();
    fs::write(root.join("secrets/dev/.env.production"), "A=1\nB=zwei\n").unwrap();
    fs::write(root.join("README.yaml"), "kein: geheimnis\n").unwrap();
    run(&sops, root, &["encrypt", "-i", "secrets/prod/api.yaml"]);
    run(&sops, root, &["encrypt", "-i", "--input-type", "dotenv", "--output-type", "dotenv", "secrets/dev/.env.production"]);

    let app = Gedoens::new(
        Arc::new(SopsCli::at(sops.clone())),
        Arc::new(FsWorkspace),
        Arc::new(SopsConfigFile),
        Arc::new(GitCli::locate()),
        Arc::new(AgeKeyRing::at(my_keys)),
    );
    let api = "secrets/prod/api.yaml";
    let env = "secrets/dev/.env.production";

    // Startseite: zwei Dateien, nichts offen. Die unverschlüsselte Datei zählt nicht.
    let overview = app.overview(root).unwrap();
    assert_eq!(overview.groups.iter().map(|g| g.files.len()).sum::<usize>(), 2);
    assert!(overview.issues.is_empty(), "{:?}", overview.issues);

    // Öffnen zeigt die Gliederung, aber keinen Wert.
    let view = app.file(root, api).unwrap();
    let labels: Vec<&str> = view.entries.iter().map(|e| e.label.as_str()).collect();
    assert_eq!(labels, ["db", "user", "port", "token"]);
    assert_eq!(view.area.as_deref(), Some("prod"));
    assert!(view.readable && view.drift.is_none());

    // Lesen, ändern, anlegen, löschen. Eine Zahl bleibt eine Zahl.
    assert_eq!(app.reveal(root, api, &key("token")).unwrap(), "abc");
    assert_eq!(app.set_value(root, api, &key("token"), "x\"y\nz").unwrap().as_deref(), Some("abc"));
    assert_eq!(app.reveal(root, api, &key("token")).unwrap(), "x\"y\nz");
    let port = vec![Step::Key("db".into()), Step::Key("port".into())];
    app.set_value(root, api, &port, "6543").unwrap();
    let plain = Command::new(&sops).current_dir(root).args(["decrypt", api]).output().unwrap();
    assert!(String::from_utf8_lossy(&plain.stdout).contains("port: 6543\n"), "die Zahl darf kein Text werden");
    assert_eq!(app.set_value(root, api, &key("neu"), "wert").unwrap(), None);
    assert_eq!(app.remove_value(root, api, &key("neu")).unwrap().as_deref(), Some("wert"));

    // Eine Datei, deren Format SOPS am Namen nicht erkennt.
    assert_eq!(app.reveal(root, env, &key("B")).unwrap(), "zwei");
    app.set_value(root, env, &key("B"), "drei").unwrap();
    assert!(fs::read_to_string(root.join(env)).unwrap().starts_with("A=ENC["));

    // Zugriff geben: Regel, Name und Datei ziehen nach.
    assert_eq!(app.grant(root, anna.as_str(), 0, Some("Anna")).unwrap(), 1);
    assert!(fs::read_to_string(root.join(".sops.yaml")).unwrap().contains(&format!("- {}  # Anna", anna.as_str())));
    let readers = app.file(root, api).unwrap().readers;
    assert_eq!(readers.len(), 2);
    assert_eq!(readers[1].name.as_deref(), Some("Anna"));
    let access = app.access(root).unwrap();
    let anna_row = access.people.iter().find(|p| p.person.recipient == anna.as_str()).unwrap();
    assert_eq!(anna_row.areas, [true, false]);

    // Zugriff nehmen: angleichen und rotieren. Wir können weiter lesen.
    assert_eq!(app.revoke(root, anna.as_str(), 0).unwrap(), 1);
    assert_eq!(app.file(root, api).unwrap().readers.len(), 1);
    assert_eq!(app.reveal(root, api, &key("token")).unwrap(), "x\"y\nz");
    assert!(app.revoke(root, me.as_str(), 0).is_err());

    // Abweichung: die Regel kennt jemanden, die Datei noch nicht.
    use gedoens_application::ports::PolicyRepository;
    SopsConfigFile.add_recipient(&root.join(".sops.yaml"), 1, &anna, None).unwrap();
    let overview = app.overview(root).unwrap();
    assert!(matches!(&overview.issues[0], Issue::Drift { file, .. } if file == env));
    app.align(root, env).unwrap();
    assert!(!app.overview(root).unwrap().issues.iter().any(|i| matches!(i, Issue::Drift { .. })));

    // Zusammenführen nach einem Konflikt.
    let git_do = |args: &[&str]| {
        let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"];
        all.extend_from_slice(args);
        run(&git, root, &all);
    };
    git_do(&["init", "-q", "-b", "main"]);
    git_do(&["add", "-A"]);
    git_do(&["commit", "-qm", "start"]);

    // Fall 1: beide ändern verschiedene Werte. Das geht ohne Rückfrage.
    git_do(&["checkout", "-qb", "anna"]);
    app.set_value(root, api, &key("token"), "von-anna").unwrap();
    git_do(&["commit", "-qam", "anna"]);
    git_do(&["checkout", "-q", "main"]);
    app.set_value(root, api, &port, "1111").unwrap();
    git_do(&["commit", "-qam", "ich"]);
    let merge = |branch: &str| {
        let args = ["-c", "user.name=t", "-c", "user.email=t@t", "merge", branch];
        let output = Command::new(&git).current_dir(root).args(args).output().unwrap();
        let said = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(said.contains("CONFLICT"), "der sops-Block sollte kollidieren: {said}");
    };
    merge("anna");

    assert!(matches!(&app.overview(root).unwrap().issues[0], Issue::Conflict { file } if file == api));
    let preview = app.merge_preview(root, api).unwrap();
    assert_eq!((preview.automatic, preview.clashes.len()), (1, 0));
    app.merge_apply(root, api, &[]).unwrap();
    assert_eq!(app.reveal(root, api, &key("token")).unwrap(), "von-anna");
    assert_eq!(app.reveal(root, api, &port).unwrap(), "1111");
    assert!(app.overview(root).unwrap().issues.iter().all(|i| !matches!(i, Issue::Conflict { .. })));
    git_do(&["commit", "-qm", "zusammengeführt"]);

    // Fall 2: beide ändern denselben Wert. Jemand muss wählen.
    git_do(&["checkout", "-qb", "anna2"]);
    app.set_value(root, api, &key("token"), "annas").unwrap();
    git_do(&["commit", "-qam", "anna2"]);
    git_do(&["checkout", "-q", "main"]);
    app.set_value(root, api, &key("token"), "meiner").unwrap();
    git_do(&["commit", "-qam", "ich2"]);
    merge("anna2");

    let preview = app.merge_preview(root, api).unwrap();
    assert_eq!(preview.clashes.len(), 1);
    assert_eq!(preview.clashes[0].ours.as_deref(), Some("meiner"));
    assert_eq!(preview.clashes[0].theirs.as_deref(), Some("annas"));
    assert!(app.merge_apply(root, api, &[]).is_err(), "ohne Entscheidung darf nichts passieren");
    app.merge_apply(root, api, &[Choice { path: key("token"), take_theirs: true }]).unwrap();
    assert_eq!(app.reveal(root, api, &key("token")).unwrap(), "annas");
}
