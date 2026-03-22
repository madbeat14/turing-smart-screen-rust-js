fn main() {
    tauri_build::build();

    #[cfg(target_os = "windows")]
    {
        let _ = embed_resource::compile("app.manifest", embed_resource::NONE);
    }
}
