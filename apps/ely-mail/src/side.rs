//! The account list down the left.

use ely_gpui_component::lists::{ListItem, SelectableList};
use ely_mail_engine::AccountId;
use gpui::{
    Context, IntoElement, ParentElement as _, Render, Styled as _, WeakEntity, Window, div,
};

use crate::{
    mail::{Mail, text},
    view::protocol,
};

pub struct Side {
    mail: WeakEntity<Mail>,
}

impl Side {
    pub fn new(mail: WeakEntity<Mail>) -> Self {
        Self { mail }
    }
}

impl Render for Side {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(mail) = self.mail.upgrade() else {
            return div();
        };
        let mail = mail.read(cx);
        let list = mail.accounts().iter().fold(
            SelectableList::new("mail-accounts").label(text(cx, "mail.accounts", &[])),
            |list, (id, account)| {
                let item = ListItem::new(("mail-account", id.0 as u64), account.address.clone())
                    .description(protocol(account, cx));
                list.row(id.0.to_string(), item)
            },
        );
        let pick = self.mail.clone();
        let list = list
            .selected(mail.picked.map(|id| id.0.to_string()))
            .on_change(move |keys, _, cx| {
                let Some(id) = keys.first().and_then(|key| key.parse().ok()) else {
                    return log::error!("mail: the account list sent no account");
                };
                pick.update(cx, |mail, cx| mail.pick(AccountId(id), cx))
                    .ok();
            })
            .size_full();
        div().size_full().p_2().child(list)
    }
}
