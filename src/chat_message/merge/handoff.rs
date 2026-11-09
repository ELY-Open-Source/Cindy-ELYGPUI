//! `remoteUserHandoff.ts`'s `confirmRemoteUsers`: a send's reserved slot retired once history has it.

use std::{collections::HashSet, rc::Rc};

use super::{Messages, get};
use crate::rich_answer::js::{self, Js};

/// `messages` itself when no reservation retires.
pub fn confirm_remote_users(messages: &Messages, confirmed: &HashSet<String>) -> Messages {
    let mut changed = false;
    let next = messages
        .iter()
        .map(|row| {
            let confirmed = get(row, "clientId")
                .and_then(Js::str)
                .is_some_and(|id| confirmed.contains(id));
            if !js::truthy(get(row, "localSendPrecedingClientIds")) || !confirmed {
                return row.clone();
            }
            let mut message = (**row).clone();
            message.shift_remove("localSendPrecedingClientIds");
            changed = true;
            Rc::new(message)
        })
        .collect();
    match changed {
        true => Rc::new(next),
        false => messages.clone(),
    }
}
