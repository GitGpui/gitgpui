use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    backend::{
        request::{Request, RequestImpl},
        MaybeAppID, MaybeWindowIdentifier, Result,
    },
    desktop::{
        file_chooser::{Choice, FileFilter},
        request::Response,
        HandleToken,
    },
    zvariant::{DeserializeDict, OwnedObjectPath, SerializeDict, Type},
    AppID, FilePath, WindowIdentifierType,
};

#[derive(Debug, Type, SerializeDict, Default)]
#[zvariant(signature = "dict")]
pub struct SelectedFiles {
    uris: Vec<url::Url>,
    choices: Option<Vec<(String, String)>>,
    // Not relevant for SaveFiles
    current_filter: Option<FileFilter>,
    // Only relevant for OpenFile
    writable: Option<bool>,
}

impl SelectedFiles {
    pub fn uri(mut self, value: url::Url) -> Self {
        self.uris.push(value);
        self
    }

    pub fn choice(mut self, choice_key: &str, choice_value: &str) -> Self {
        self.choices
            .get_or_insert_with(Vec::new)
            .push((choice_key.to_owned(), choice_value.to_owned()));
        self
    }

    pub fn current_filter(mut self, value: impl Into<Option<FileFilter>>) -> Self {
        self.current_filter = value.into();
        self
    }

    pub fn writable(mut self, value: impl Into<Option<bool>>) -> Self {
        self.writable = value.into();
        self
    }
}
// TODO: We should de-duplicate those types
// but we will have to figure out how to handle handle_token
// as if we set it to Option<T>, the Default would no longer
// generate a random value, breaking some of the infrastructure we had
#[derive(DeserializeDict, Type, Debug)]
#[zvariant(signature = "dict")]
pub struct OpenFileOptions {
    accept_label: Option<String>,
    modal: Option<bool>,
    multiple: Option<bool>,
    directory: Option<bool>,
    filters: Option<Vec<FileFilter>>,
    current_filter: Option<FileFilter>,
    choices: Option<Vec<Choice>>,
    current_folder: Option<FilePath>,
}

impl OpenFileOptions {
    pub fn accept_label(&self) -> Option<&str> {
        self.accept_label.as_deref()
    }

    pub fn modal(&self) -> Option<bool> {
        self.modal
    }

    pub fn multiple(&self) -> Option<bool> {
        self.multiple
    }

    pub fn directory(&self) -> Option<bool> {
        self.directory
    }

    pub fn filters(&self) -> &[FileFilter] {
        self.filters.as_deref().unwrap_or_default()
    }

    pub fn current_filter(&self) -> Option<&FileFilter> {
        self.current_filter.as_ref()
    }

    pub fn choices(&self) -> &[Choice] {
        self.choices.as_deref().unwrap_or_default()
    }

    pub fn current_folder(&self) -> Option<&FilePath> {
        self.current_folder.as_ref()
    }
}

#[derive(DeserializeDict, Type, Debug)]
#[zvariant(signature = "dict")]
pub struct SaveFileOptions {
    accept_label: Option<String>,
    modal: Option<bool>,
    multiple: Option<bool>,
    filters: Option<Vec<FileFilter>>,
    current_filter: Option<FileFilter>,
    choices: Option<Vec<Choice>>,
    current_name: Option<String>,
    current_folder: Option<FilePath>,
    current_file: Option<FilePath>,
}

impl SaveFileOptions {
    pub fn accept_label(&self) -> Option<&str> {
        self.accept_label.as_deref()
    }

    pub fn modal(&self) -> Option<bool> {
        self.modal
    }

    pub fn multiple(&self) -> Option<bool> {
        self.multiple
    }

    pub fn filters(&self) -> &[FileFilter] {
        self.filters.as_deref().unwrap_or_default()
    }

    pub fn current_filter(&self) -> Option<&FileFilter> {
        self.current_filter.as_ref()
    }

    pub fn choices(&self) -> &[Choice] {
        self.choices.as_deref().unwrap_or_default()
    }

    pub fn current_folder(&self) -> Option<&FilePath> {
        self.current_folder.as_ref()
    }

    pub fn current_file(&self) -> Option<&FilePath> {
        self.current_file.as_ref()
    }

    pub fn current_name(&self) -> Option<&str> {
        self.current_name.as_deref()
    }
}

#[derive(DeserializeDict, Type, Debug)]
#[zvariant(signature = "dict")]
pub struct SaveFilesOptions {
    accept_label: Option<String>,
    modal: Option<bool>,
    choices: Option<Vec<Choice>>,
    current_folder: Option<FilePath>,
    files: Option<Vec<FilePath>>,
}

impl SaveFilesOptions {
    pub fn accept_label(&self) -> Option<&str> {
        self.accept_label.as_deref()
    }

    pub fn modal(&self) -> Option<bool> {
        self.modal
    }

    pub fn choices(&self) -> &[Choice] {
        self.choices.as_deref().unwrap_or_default()
    }

    pub fn current_folder(&self) -> Option<&FilePath> {
        self.current_folder.as_ref()
    }

    pub fn files(&self) -> &[FilePath] {
        self.files.as_deref().unwrap_or_default()
    }
}

#[async_trait]
pub trait FileChooserImpl: RequestImpl {
    async fn open_file(
        &self,
        token: HandleToken,
        app_id: Option<AppID>,
        window_identifier: Option<WindowIdentifierType>,
        title: &str,
        options: OpenFileOptions,
    ) -> Result<SelectedFiles>;

    async fn save_file(
        &self,
        token: HandleToken,
        app_id: Option<AppID>,
        window_identifier: Option<WindowIdentifierType>,
        title: &str,
        options: SaveFileOptions,
    ) -> Result<SelectedFiles>;

    async fn save_files(
        &self,
        token: HandleToken,
        app_id: Option<AppID>,
        window_identifier: Option<WindowIdentifierType>,
        title: &str,
        options: SaveFilesOptions,
    ) -> Result<SelectedFiles>;
}

pub(crate) struct FileChooserInterface {
    imp: Arc<dyn FileChooserImpl>,
    cnx: zbus::Connection,
}

impl FileChooserInterface {
    pub fn new(imp: Arc<dyn FileChooserImpl>, cnx: zbus::Connection) -> Self {
        Self { imp, cnx }
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooserInterface {
    #[zbus(property(emits_changed_signal = "const"), name = "version")]
    fn version(&self) -> u32 {
        4
    }

    #[zbus(out_args("response", "results"))]
    async fn open_file(
        &self,
        handle: OwnedObjectPath,
        app_id: MaybeAppID,
        window_identifier: MaybeWindowIdentifier,
        title: String,
        options: OpenFileOptions,
    ) -> Result<Response<SelectedFiles>> {
        let imp = Arc::clone(&self.imp);

        Request::spawn(
            "FileChooser::OpenFile",
            &self.cnx,
            handle.clone(),
            Arc::clone(&self.imp),
            async move {
                imp.open_file(
                    HandleToken::try_from(&handle).unwrap(),
                    app_id.inner(),
                    window_identifier.inner(),
                    &title,
                    options,
                )
                .await
            },
        )
        .await
    }

    #[zbus(out_args("response", "results"))]
    async fn save_file(
        &self,
        handle: OwnedObjectPath,
        app_id: MaybeAppID,
        window_identifier: MaybeWindowIdentifier,
        title: String,
        options: SaveFileOptions,
    ) -> Result<Response<SelectedFiles>> {
        let imp = Arc::clone(&self.imp);

        Request::spawn(
            "FileChooser::SaveFile",
            &self.cnx,
            handle.clone(),
            Arc::clone(&self.imp),
            async move {
                imp.save_file(
                    HandleToken::try_from(&handle).unwrap(),
                    app_id.inner(),
                    window_identifier.inner(),
                    &title,
                    options,
                )
                .await
            },
        )
        .await
    }

    #[zbus(out_args("response", "results"))]
    async fn save_files(
        &self,
        handle: OwnedObjectPath,
        app_id: MaybeAppID,
        window_identifier: MaybeWindowIdentifier,
        title: String,
        options: SaveFilesOptions,
    ) -> Result<Response<SelectedFiles>> {
        let imp = Arc::clone(&self.imp);

        Request::spawn(
            "FileChooser::SaveFiles",
            &self.cnx,
            handle.clone(),
            Arc::clone(&self.imp),
            async move {
                imp.save_files(
                    HandleToken::try_from(&handle).unwrap(),
                    app_id.inner(),
                    window_identifier.inner(),
                    &title,
                    options,
                )
                .await
            },
        )
        .await
    }
}
