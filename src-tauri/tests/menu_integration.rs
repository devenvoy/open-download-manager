use open_download_manager_lib::presentation::menu::menu_id_to_command;

#[test]
fn menu_commands_map_to_all_required_downloader_events() {
    let expected = vec![
        ("cmd_add", "add"),
        ("cmd_add_batch", "add"),
        ("cmd_paste", "paste"),
        ("cmd_details", "details"),
        ("cmd_remove", "remove"),
        ("cmd_remove_completed", "removeCompleted"),
        ("cmd_delete_completed", "removeCompleted"),
        ("cmd_pause_all", "pauseAll"),
        ("cmd_stop_all", "pauseAll"),
        ("cmd_pause_all_queue", "pauseAll"),
        ("cmd_resume_all", "resumeAll"),
        ("cmd_find", "find"),
        ("cmd_scheduler", "scheduler"),
        ("cmd_settings_speed", "settings"),
        ("cmd_settings_options", "settings"),
        ("cmd_integration", "integration"),
        ("cmd_all", "all"),
    ];

    for (menu_id, command) in expected {
        assert_eq!(
            menu_id_to_command(menu_id),
            Some(command),
            "Failed mapping for {menu_id}"
        );
    }

    // Non-downloader:command actions return None (they trigger native flows)
    assert_eq!(menu_id_to_command("export_txt"), None);
    assert_eq!(menu_id_to_command("import_txt"), None);
    assert_eq!(menu_id_to_command("about"), None);
    assert_eq!(menu_id_to_command("check_updates"), None);
    assert_eq!(menu_id_to_command("exit"), None);
    assert_eq!(menu_id_to_command("unknown_id"), None);
}

#[test]
fn all_12_required_command_strings_are_represented() {
    let required = [
        "add",
        "paste",
        "details",
        "remove",
        "removeCompleted",
        "pauseAll",
        "resumeAll",
        "find",
        "scheduler",
        "settings",
        "integration",
        "all",
    ];

    let all_menu_ids = [
        "cmd_add",
        "cmd_paste",
        "cmd_details",
        "cmd_remove",
        "cmd_remove_completed",
        "cmd_pause_all",
        "cmd_resume_all",
        "cmd_find",
        "cmd_scheduler",
        "cmd_settings_options",
        "cmd_integration",
        "cmd_all",
    ];

    let mapped: Vec<&str> = all_menu_ids
        .iter()
        .filter_map(|id| menu_id_to_command(id))
        .collect();

    for req in required {
        assert!(
            mapped.contains(&req),
            "Required command '{req}' is missing from menu event mappings"
        );
    }
}
