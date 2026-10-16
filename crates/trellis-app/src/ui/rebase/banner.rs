//! The top of the rebase banner on the WIP row: "<name> in progress · step N of M", the
//! stopped commit (short SHA, summary, and why it stopped), and the autostash note. The
//! conflicted-files row and the buttons below it are drawn by `details::wip_panel`.

use crate::state::rebase_banner::{STASH_NOTE, lines};
use crate::ui::theme::color;
use eframe::egui::{self, FontId, RichText, TextFormat, text::LayoutJob};
use trellis_core::rebase::RebaseProgress;

pub fn progress(ui: &mut egui::Ui, p: &RebaseProgress, name: &str, conflicts_left: bool) {
    let text = lines(p, name, conflicts_left);
    ui.label(RichText::new(text.title).strong());
    if let Some(stop) = &text.stop {
        let body = egui::TextStyle::Body.resolve(ui.style());
        let mono = FontId::monospace(body.size);
        let plain = |job: &mut LayoutJob, s: &str| {
            job.append(s, 0.0, TextFormat { font_id: body.clone(), color: color::text(), ..Default::default() })
        };
        let mut job = LayoutJob::default();
        plain(&mut job, stop.before);
        job.append(&stop.sha, 4.0, TextFormat { font_id: mono, color: color::weak(), ..Default::default() });
        if !stop.summary.is_empty() {
            plain(&mut job, " ");
            plain(&mut job, &stop.summary);
        }
        plain(&mut job, stop.after);
        ui.add(egui::Label::new(job).wrap());
    }
    if text.stash_note {
        ui.label(RichText::new(STASH_NOTE).color(color::weak()));
    }
}
