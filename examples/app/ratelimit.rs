use carapax::{
    ratelimit::{
        nonzero, DirectRateLimitPredicate, Jitter, KeyChat, KeyChatUser, KeyUser, KeyedRateLimitPredicate, Quota,
    },
    Dispatcher, PredicateExt,
};
use std::time::Duration;

pub fn setup(dispatcher: &mut Dispatcher, strategy: &str) {
    let quota = Quota::with_period(Duration::from_secs(5))
        .expect("Failed to create quota")
        .allow_burst(nonzero!(1u32));
    let jitter = Jitter::up_to(Duration::from_secs(5));
    match strategy {
        "direct_discard" => {
            dispatcher.add_handler(handler.predicate(DirectRateLimitPredicate::discard(quota)));
        }
        "direct_wait" => {
            dispatcher.add_handler(handler.predicate(DirectRateLimitPredicate::wait(quota)));
        }
        "direct_wait_with_jitter" => {
            dispatcher.add_handler(handler.predicate(DirectRateLimitPredicate::wait_with_jitter(quota, jitter)));
        }
        "keyed_discard" => {
            dispatcher.add_handler(handler.predicate(<KeyedRateLimitPredicate<KeyChat, _, _>>::discard(quota)));
        }
        "keyed_wait" => {
            dispatcher.add_handler(handler.predicate(<KeyedRateLimitPredicate<KeyUser, _, _>>::wait(quota)));
        }
        "keyed_wait_with_jitter" => {
            dispatcher.add_handler(
                handler.predicate(<KeyedRateLimitPredicate<KeyChatUser, _, _>>::wait_with_jitter(
                    quota, jitter,
                )),
            );
        }
        key => panic!("Unknown ratelimit stragey: {}", key),
    }
}

async fn handler(_: ()) {
    // TODO: get rid of handler, convert predicates into handlers
}