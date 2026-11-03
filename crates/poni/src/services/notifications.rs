use crate::events::{AppEvent, EventSink};
use crate::state::NotifyAccess;
use poni_core::notifications::Seen;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use windows::core::Result;
use windows::Foundation::{AsyncOperationCompletedHandler, AsyncStatus, EventRegistrationToken, TypedEventHandler};
use windows::UI::Notifications::Management::{UserNotificationListener, UserNotificationListenerAccessStatus};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotification, UserNotificationChangedEventArgs, UserNotificationChangedKind};

/// What the listener tells the UI thread.
#[derive(Debug)]
pub enum NotifyEvent {
    /// What Windows allows now (the app passes it to [`Listener::follow`]).
    Access(NotifyAccess),
    /// The apps with notifications in Windows' list when listening began
    /// (id, name), so the page lists them before they send a new one.
    Apps(Vec<(String, String)>),
    /// A new notification.
    Seen(Seen),
}

/// The listener while the add-on is on.
pub struct Listener {
    listener: UserNotificationListener,
    token: Option<EventRegistrationToken>,
    sink: EventSink,
    running: Arc<AtomicBool>,
}

impl Listener {
    /// Look at what Windows allows and listen if it lets us; `ask`: the
    /// person just switched the add-on on, so Windows' question comes now
    /// (at a start with the add-on on, the page offers Allow instead).
    /// None, with the reason sent, when this copy can't listen at all.
    pub fn start(sink: EventSink, ask: bool) -> Option<Self> {
        let listener = match UserNotificationListener::Current() {
            Ok(l) => l,
            Err(e) => {
                crate::pwarn!("notifications: no listener: {e}");
                sink.send(AppEvent::Notify(NotifyEvent::Access(NotifyAccess::Failed)));
                return None;
            }
        };
        let mut me = Self {
            listener,
            token: None,
            sink,
            running: Arc::new(AtomicBool::new(false)),
        };
        match me.access() {
            NotifyAccess::Ask if ask => me.ask(),
            a => me.tell(a),
        }
        Some(me)
    }

    /// What Windows says now (`Ask` for "not asked yet").
    fn access(&self) -> NotifyAccess {
        match self.listener.GetAccessStatus() {
            Ok(s) => access_of(s, false),
            Err(e) => {
                crate::pwarn!("notifications: access unreadable: {e}");
                NotifyAccess::Failed
            }
        }
    }

    /// Ask Windows (its question). On the UI thread: Windows wants it there.
    /// The answer comes back as `NotifyEvent::Access`.
    pub fn ask(&mut self) {
        let op = match self.listener.RequestAccessAsync() {
            Ok(op) => op,
            Err(e) => {
                crate::pwarn!("notifications: couldn't ask Windows: {e}");
                self.tell(NotifyAccess::Failed);
                return;
            }
        };
        self.sink.send(AppEvent::Notify(NotifyEvent::Access(NotifyAccess::Asking)));
        let sink = self.sink.clone();
        let done = op.SetCompleted(&AsyncOperationCompletedHandler::new(move |op, status| {
            let access = match (status, op.map(|o| o.GetResults())) {
                (AsyncStatus::Completed, Some(Ok(s))) => access_of(s, true),
                _ => NotifyAccess::Failed,
            };
            crate::pinfo!("notifications: Windows answered {access:?}");
            sink.send(AppEvent::Notify(NotifyEvent::Access(access)));
            Ok(())
        }));
        if let Err(e) = done {
            crate::pwarn!("notifications: no answer will come: {e}");
            self.tell(NotifyAccess::Failed);
        }
    }

    /// Look again (the page opened): the person may have changed it in
    /// Windows' settings meanwhile.
    pub fn check(&mut self) {
        let a = self.access();
        self.tell(a);
    }

    /// Pass what Windows allows to the UI thread, which hands it to `follow`.
    fn tell(&self, access: NotifyAccess) {
        self.sink.send(AppEvent::Notify(NotifyEvent::Access(access)));
    }

    /// Do what Windows allows (the app calls it for each `NotifyEvent::Access`):
    /// listen once it's allowed, stop if it no longer is. Sends nothing back
    /// unless listening fails.
    pub fn follow(&mut self, access: NotifyAccess) {
        match access {
            NotifyAccess::Allowed => self.listen(),
            NotifyAccess::Asking => {}
            _ => self.stop(),
        }
    }

    /// Start hearing new notifications.
    /// In packaged apps, NotificationChanged event works.
    /// In unpackaged apps, Windows returns 0x80070490 on event registration,
    /// so we seamlessly fall back to lightweight async polling of toast notifications!
    fn listen(&mut self) {
        if self.running.load(Ordering::SeqCst) || self.token.is_some() {
            return;
        }

        // Try push event first
        let sink = self.sink.clone();
        let handler = TypedEventHandler::<UserNotificationListener, UserNotificationChangedEventArgs>::new(move |sender, args| {
            let (Some(listener), Some(args)) = (sender.as_ref(), args.as_ref()) else { return Ok(()) };
            if args.ChangeKind()? != UserNotificationChangedKind::Added {
                return Ok(());
            }
            match listener.GetNotification(args.UserNotificationId()?).and_then(|n| read(&n)) {
                Ok(seen) => sink.send(AppEvent::Notify(NotifyEvent::Seen(seen))),
                Err(e) => crate::pinfo!("notifications: one skipped: {e}"),
            }
            Ok(())
        });

        match self.listener.NotificationChanged(&handler) {
            Ok(token) => {
                crate::pinfo!("notifications: listening via NotificationChanged event");
                self.token = Some(token);
            }
            Err(_e) => {
                // Windows requires package identity for push event, but allows pull polling!
                crate::pinfo!("notifications: push event registration unavailable; using polling fallback");
                self.start_polling();
            }
        }

        // Apps list and historical notifications are populated inside the polling thread.
    }

    /// Background polling fallback for unpackaged execution (checks for new toast notifications every 1.5s).
    /// Creates a fresh UserNotificationListener::Current() inside the thread to avoid COM thread marshalling errors.
    fn start_polling(&mut self) {
        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);
        let sink = self.sink.clone();

        let _ = std::thread::Builder::new().name("poni-notify-poll".into()).spawn(move || {
            // Get a fresh listener on this thread (COM WinRT objects must be used on the thread they were created)
            let listener = match UserNotificationListener::Current() {
                Ok(l) => l,
                Err(e) => {
                    crate::pwarn!("notifications: poll thread: can't get listener: {e}");
                    return;
                }
            };

            // Seed known notification IDs so historical ones don't trigger alerts
            let mut known_ids: HashSet<u32> = HashSet::new();
            if let Ok(list) = listener.GetNotificationsAsync(NotificationKinds::Toast).and_then(|op| op.get()) {
                if let Ok(size) = list.Size() {
                    for i in 0..size {
                        if let Ok(n) = list.GetAt(i) {
                            if let Ok(id) = n.Id() {
                                known_ids.insert(id);
                            }
                        }
                    }
                }
                crate::pinfo!("notifications: polling started; {} existing notifications seeded", known_ids.len());
            }

            // Also send apps list from this thread's fresh listener
            let mut apps: Vec<(String, String)> = Vec::new();
            if let Ok(list) = listener.GetNotificationsAsync(NotificationKinds::Toast).and_then(|op| op.get()) {
                if let Ok(size) = list.Size() {
                    for i in 0..size {
                        if let Ok(n) = list.GetAt(i) {
                            if let Ok((id, name)) = app_of(&n) {
                                if !apps.iter().any(|a| a.0 == id) {
                                    apps.push((id, name));
                                }
                            }
                        }
                    }
                }
            }
            if !apps.is_empty() {
                sink.send(AppEvent::Notify(NotifyEvent::Apps(apps)));
            }

            while running.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1500));
                if !running.load(Ordering::SeqCst) {
                    break;
                }

                if let Ok(list) = listener.GetNotificationsAsync(NotificationKinds::Toast).and_then(|op| op.get()) {
                    if let Ok(size) = list.Size() {
                        let mut current_ids: HashSet<u32> = HashSet::new();
                        for i in 0..size {
                            if let Ok(n) = list.GetAt(i) {
                                if let Ok(id) = n.Id() {
                                    current_ids.insert(id);
                                    if !known_ids.contains(&id) {
                                        match read(&n) {
                                            Ok(seen) => {
                                                crate::pinfo!("notifications: new notification from {}", seen.app);
                                                sink.send(AppEvent::Notify(NotifyEvent::Seen(seen)));
                                            }
                                            Err(e) => crate::pinfo!("notifications: one skipped: {e}"),
                                        }
                                    }
                                }
                            }
                        }
                        known_ids = current_ids;
                    }
                }
            }
            crate::pinfo!("notifications: polling thread stopped");
        });
    }

    /// Stop hearing them (the add-on off, or Windows no longer allows it).
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(token) = self.token.take() {
            if let Err(e) = self.listener.RemoveNotificationChanged(token) {
                crate::pwarn!("notifications: stopping failed: {e}");
            }
            crate::pinfo!("notifications: stopped listening");
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Windows' status as the page shows it; `asked`: the answer to our
/// question, where "unspecified" means it went unanswered.
fn access_of(s: UserNotificationListenerAccessStatus, asked: bool) -> NotifyAccess {
    match s {
        UserNotificationListenerAccessStatus::Allowed => NotifyAccess::Allowed,
        UserNotificationListenerAccessStatus::Denied => NotifyAccess::Denied,
        UserNotificationListenerAccessStatus::Unspecified if asked => NotifyAccess::Unanswered,
        UserNotificationListenerAccessStatus::Unspecified => NotifyAccess::Ask,
        _ => NotifyAccess::Failed,
    }
}

/// A notification's app (id, display name).
fn app_of(n: &UserNotification) -> Result<(String, String)> {
    let info = n.AppInfo()?;
    let id = info.AppUserModelId()?.to_string();
    let name = info.DisplayInfo().and_then(|d| d.DisplayName()).map(|h| h.to_string()).unwrap_or_default();
    Ok((id, name))
}

/// A notification's app and its lines of text (a toast's text elements).
fn read(n: &UserNotification) -> Result<Seen> {
    let (app_id, app) = app_of(n)?;
    let binding = n.Notification()?.Visual()?.GetBinding(&KnownNotificationBindings::ToastGeneric()?)?;
    let texts = binding.GetTextElements()?;
    let mut lines = Vec::new();
    for i in 0..texts.Size()? {
        if let Ok(t) = texts.GetAt(i).and_then(|t| t.Text()) {
            lines.push(t.to_string());
        }
    }
    Ok(Seen { app_id, app, lines })
}

/// For the self-test: what this copy can do, without asking Windows anything.
/// Err when a packaged copy can't reach the listener.
pub fn describe() -> std::result::Result<String, String> {
    let status = UserNotificationListener::Current().and_then(|l| l.GetAccessStatus()).map_err(|e| format!("listener unavailable: {e}"))?;
    Ok(format!("Windows says {:?}", access_of(status, false)))
}
