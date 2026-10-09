use ely_gpui_component::{
    feedback::{Toast, ToastViewport, Toaster},
    navigation::{Command as PaletteCommand, CommandPalette},
    primitives::{Severity, raise},
};
use ely_platform::AppId;
use std::path::{Path, PathBuf};

use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    Keystroke, ParentElement, Render, SharedString, Styled as _, Window, div,
    prelude::FluentBuilder as _,
};

use super::Shell;
use crate::commands::{
    self, Export, New, Open, OpenElyAgent, OpenElyDoc, OpenElyMail, OpenElyMark, OpenElyPdf,
    OpenElyShow, OpenElyTable, OpenElyTree, Print, Save, SaveAs, ToggleCommandPalette,
};

/// The shared root: shell, file commands, command palette.
pub struct Root {
    app: AppId,
    focus: FocusHandle,
    palette: bool,
    recent: Vec<SharedString>,
    last: Option<SharedString>,
    toaster: Entity<Toaster>,
    content: Option<gpui::AnyView>,
    document: bool,
    file: &'static [&'static commands::Command],
}

/// A window with only its frame.
pub fn blank(app: AppId, window: &mut Window, cx: &mut App) -> Entity<Root> {
    root(app, None, window, cx)
}

pub fn root(
    app: AppId,
    content: Option<gpui::AnyView>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<Root> {
    build(app, content, false, &commands::FILE, Vec::new(), window, cx)
}

/// A tool window: File has New and Open; `menus` follow.
pub fn tool_root(
    app: AppId,
    content: gpui::AnyView,
    menus: Vec<gpui::Menu>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<Root> {
    build(
        app,
        Some(content),
        false,
        &commands::OPENING,
        menus,
        window,
        cx,
    )
}

/// A document window: File holds `file`; `menus` follow.
pub fn document_root(
    app: AppId,
    document: gpui::AnyView,
    file: &'static [&'static commands::Command],
    menus: Vec<gpui::Menu>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<Root> {
    build(app, Some(document), true, file, menus, window, cx)
}

/// Replaces a document window's menus.
pub fn set_document_menus(
    app: AppId,
    file: &'static [&'static commands::Command],
    menus: Vec<gpui::Menu>,
    cx: &mut App,
) {
    cx.set_menus(commands::menus(app, file, true, menus, cx));
}

fn build(
    app: AppId,
    content: Option<gpui::AnyView>,
    document: bool,
    file: &'static [&'static commands::Command],
    menus: Vec<gpui::Menu>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<Root> {
    cx.set_menus(commands::menus(app, file, document, menus, cx));
    cx.new(|cx| {
        let focus = cx.focus_handle();
        // A document takes focus itself.
        if !document {
            window.focus(&focus, cx);
        }
        Root {
            app,
            focus,
            palette: false,
            recent: Vec::new(),
            last: None,
            toaster: cx.new(|_| Toaster::default()),
            content,
            document,
            file,
        }
    })
}

impl Root {
    pub fn palette_open(&self) -> bool {
        self.palette
    }

    pub fn recent(&self) -> &[SharedString] {
        &self.recent
    }

    pub fn last(&self) -> Option<&SharedString> {
        self.last.as_ref()
    }

    fn attempt(&mut self, id: &'static str, cx: &mut Context<Self>) {
        if let Some(reason) = commands::blocked(id, self.document, cx) {
            log::info!("command: {id} blocked: {reason}");
            self.last = Some(format!("{id} blocked").into());
            self.toaster.update(cx, |toaster, cx| {
                toaster.push(Toast::new(reason).severity(Severity::Warning), cx);
            });
            cx.notify();
            return;
        }
        self.ran(id, cx);
    }

    /// Shows `text` in the window's toaster.
    pub fn toast(
        &mut self,
        text: impl Into<SharedString>,
        severity: Severity,
        cx: &mut Context<Self>,
    ) {
        self.toaster.update(cx, |toaster, cx| {
            toaster.push(Toast::new(text.into()).severity(severity), cx);
        });
    }

    /// A toast whose button opens the output `path` (§16).
    pub fn toast_output(
        &mut self,
        text: impl Into<SharedString>,
        severity: Severity,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let root = cx.weak_entity();
        let open = crate::i18n::text(cx, "toast.open", &[]);
        let toast = Toast::new(text.into())
            .severity(severity)
            .action(open, move |_, cx| {
                let (root, path) = (root.clone(), path.clone());
                // Opened off the UI thread; a failure comes back.
                cx.spawn(async move |cx| {
                    let opened = cx.background_spawn(async move { view(&path) }).await;
                    if let Err(error) = opened {
                        log::error!("apps: the output did not open: {error}");
                        cx.update(|cx| {
                            let said = crate::i18n::text(cx, "toast.open_failed", &[("error", &error.to_string())]);
                            root.update(cx, |root, cx| root.toast(said, Severity::Danger, cx)).ok();
                        });
                    }
                })
                .detach();
            });
        self.toaster.update(cx, |toaster, cx| {
            toaster.push(toast, cx);
        });
    }

    /// Starts another Ely app; a failure says why.
    fn switch(&mut self, app: AppId, cx: &mut Context<Self>) {
        cx.spawn(async move |root, cx| {
            let opened = cx.background_spawn(async move { ely_platform::apps::open(app, &[]) }).await;
            if let Err(error) = opened {
                log::error!("apps: {} did not open: {error}", app.name());
                root.update(cx, |root, cx| {
                    let said = crate::i18n::text(cx, "menu.app.switch_failed", &[("app", app.name()), ("error", &error.to_string())]);
                    root.toast(said, Severity::Danger, cx);
                })
                .ok();
            }
        })
        .detach();
    }

    pub fn close_palette(&mut self, cx: &mut Context<Self>) {
        self.palette = false;
        cx.notify();
    }

    fn ran(&mut self, id: &'static str, cx: &mut Context<Self>) {
        log::info!("command: {id} ran in {}", self.app.name());
        self.last = Some(id.into());
        self.recent.retain(|value| value != id);
        self.recent.insert(0, id.into());
        self.recent.truncate(5);
        cx.notify();
    }
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let body = match &self.content {
            Some(view) => view.clone().into_any_element(),
            None => Shell::new(self.app.name(), &self.focus).into_any_element(),
        };
        let has = |id| self.file.iter().any(|command| command.id == id);
        let frame = div()
            .size_full()
            .key_context(commands::CONTEXT)
            .on_action(cx.listener(|this, _: &New, _, cx| this.attempt("file.new", cx)))
            .on_action(cx.listener(|this, _: &Open, _, cx| this.attempt("file.open", cx)))
            .when(has("file.save"), |frame| {
                frame
                    .on_action(cx.listener(|this, _: &Save, _, cx| this.attempt("file.save", cx)))
                    .on_action(
                        cx.listener(|this, _: &SaveAs, _, cx| this.attempt("file.save_as", cx)),
                    )
                    .on_action(
                        cx.listener(|this, _: &Export, _, cx| this.attempt("file.export", cx)),
                    )
                    .on_action(cx.listener(|this, _: &Print, _, cx| this.attempt("file.print", cx)))
            })
            .on_action(cx.listener(|this, _: &OpenElyDoc, _, cx| this.switch(AppId::Doc, cx)))
            .on_action(cx.listener(|this, _: &OpenElyTable, _, cx| this.switch(AppId::Table, cx)))
            .on_action(cx.listener(|this, _: &OpenElyShow, _, cx| this.switch(AppId::Show, cx)))
            .on_action(cx.listener(|this, _: &OpenElyPdf, _, cx| this.switch(AppId::Pdf, cx)))
            .on_action(cx.listener(|this, _: &OpenElyMail, _, cx| this.switch(AppId::Mail, cx)))
            .on_action(cx.listener(|this, _: &OpenElyTree, _, cx| this.switch(AppId::Tree, cx)))
            .on_action(cx.listener(|this, _: &OpenElyMark, _, cx| this.switch(AppId::Mark, cx)))
            .on_action(cx.listener(|this, _: &OpenElyAgent, _, cx| this.switch(AppId::Agent, cx)))
            .on_action(cx.listener(|this, _: &ToggleCommandPalette, window, cx| {
                if this.palette {
                    window.on_next_frame(|window, cx| {
                        window.dispatch_keystroke(
                            Keystroke {
                                key: "escape".into(),
                                ..Default::default()
                            },
                            cx,
                        );
                    });
                } else {
                    this.palette = true;
                    cx.notify();
                }
            }))
            .child(body);
        let frame = frame.child(raise(
            "toasts",
            ToastViewport::new("toast-viewport", &self.toaster),
        ));
        if !self.palette {
            return frame.into_any_element();
        }
        let palette = CommandPalette::new("command-palette", move |_, cx| {
            this.update(cx, |this, cx| this.close_palette(cx));
        })
        .group(
            crate::i18n::text(cx, "palette.group.file", &[]),
            self.file.iter().map(|c| {
                PaletteCommand::new(
                    c.id,
                    crate::i18n::text(cx, &format!("command.{}", c.id), &[]),
                )
                .keys(c.keys)
            }),
        )
        .recent(self.recent.clone())
        .on_run(move |value, window, cx| commands::run(value, window, cx));
        frame.child(raise("palette", palette)).into_any_element()
    }
}

/// PDFs open in ElyPDF if installed; others, the system's app.
fn view(path: &Path) -> std::io::Result<()> {
    let pdf = path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"));
    if pdf && ely_platform::apps::installed(AppId::Pdf) {
        return ely_platform::apps::open(AppId::Pdf, &[path.to_path_buf()]);
    }
    ely_platform::apps::open_default(path)
}
