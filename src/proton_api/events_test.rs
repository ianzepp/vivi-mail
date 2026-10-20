use super::*;

#[test]
fn missing_message_action_does_not_default_to_delete() {
    let err = serde_json::from_str::<ProtonMessageEvent>(r#"{"ID":"proton-id"}"#).unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("Action") || message.contains("missing field"),
        "missing Action must fail parse, not default to Delete: {message}"
    );
}

#[test]
fn explicit_delete_action_is_zero() {
    let event: ProtonMessageEvent =
        serde_json::from_str(r#"{"ID":"proton-id","Action":0}"#).unwrap();
    assert_eq!(event.action, ProtonEventAction::Delete);
}

#[test]
fn unknown_action_is_treated_as_update_not_delete() {
    let event: ProtonMessageEvent =
        serde_json::from_str(r#"{"ID":"proton-id","Action":99}"#).unwrap();
    assert_eq!(event.action, ProtonEventAction::Update);
}

#[test]
fn event_response_parses_message_actions() {
    let response: EventResponse = serde_json::from_str(
        r#"{"Event":{"EventID":"event-2","Messages":[{"ID":"proton-id","Action":1,"Message":{"ID":"proton-id","Subject":"subject","LabelIDs":["0"]}}]},"More":false}"#,
    )
    .unwrap();

    assert!(!response.more);
    assert_eq!(response.event.event_id, "event-2");
    assert_eq!(response.event.messages[0].id, "proton-id");
    assert_eq!(response.event.messages[0].action, ProtonEventAction::Create);
    assert_eq!(
        response.event.messages[0].message.as_ref().unwrap().subject,
        "subject"
    );
}
