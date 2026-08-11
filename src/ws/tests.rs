#[cfg(test)]
mod tests {
    use crate::ws::WsState;

    #[tokio::test]
    async fn test_add_and_remove_user() {
        let ws_state = WsState::new();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

        let user = crate::ws::ConnectedUser {
            user_id: 1,
            username: "test_user".to_string(),
            channel_type: "dm".to_string(),
            channel_id: 2,
        };

        ws_state.add_user_to_channel("dm:1:2".to_string(), user, tx).await;
        let users = ws_state.get_channel_users("dm:1:2").await;
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].user_id, 1);

        ws_state.remove_user_from_channel("dm:1:2", 1).await;
        let users = ws_state.get_channel_users("dm:1:2").await;
        assert_eq!(users.len(), 0);
    }
}
