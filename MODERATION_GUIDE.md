# Orbifon Moderation System Guide 🛡️

## Overview

Orbifon includes a complete moderation system with:
- User-facing reporting functionality
- Moderator dashboard
- Content safety checks
- Rate limiting
- Reputation tracking
- Full audit logging

## API Endpoints

### User-Facing Reports

#### POST `/api/reports`
**Submit a content or user report**

```bash
curl -X POST http://localhost:8080/api/reports \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "report_type": "spam",
    "reason": "This post contains spam links",
    "post_id": 123,
    "reported_user_id": null
  }'
```

**Report Types:**
- `spam` - Spam content
- `harassment` - Harassment or bullying
- `nsfw` - Adult content
- `violence` - Violent content
- `misinformation` - False information

**Fields:**
- `report_type` (required): Type of report
- `reason` (required): Detailed reason (1-500 chars)
- `post_id` (optional): Post ID if reporting a post
- `comment_id` (optional): Comment ID if reporting a comment
- `message_id` (optional): Message ID if reporting a message
- `reported_user_id` (optional): User ID if reporting a user

**Limits:**
- Users can submit max 1 report per hour
- Must report either content or a user

### Moderator Dashboard

#### GET `/api/mod/reports/pending`
**List pending reports (Moderator only)**

```bash
curl http://localhost:8080/api/mod/reports/pending \
  -H "Authorization: Bearer <moderator_token>" \
  -H "Content-Type: application/json"
```

**Query Parameters:**
- `limit` (default: 25, max: 100)
- `offset` (default: 0)
- `sort` (default: "newest", options: "newest", "oldest")
- `report_type` (filter by type)

**Response:**
```json
[
  {
    "id": 1,
    "reporter_username": "User 123",
    "reported_content_type": "post",
    "report_type": "spam",
    "reason": "This post contains spam links",
    "status": "pending",
    "created_at": "2026-08-11T12:00:00Z"
  }
]
```

#### PATCH `/api/mod/reports/:id/review`
**Review and action a report (Moderator only)**

```bash
curl -X PATCH http://localhost:8080/api/mod/reports/1/review \
  -H "Authorization: Bearer <moderator_token>" \
  -H "Content-Type: application/json" \
  -d '{
    "status": "actioned",
    "action_taken": "content_removed"
  }'
```

**Status:**
- `dismissed` - Report is not valid
- `actioned` - Action was taken

**Actions (when status=actioned):**
- `content_removed` - Delete the reported content
- `user_suspended` - Suspend user for 7 days
- `warning` - Send warning to user

## Setting Up Moderators

1. **Login to MySQL:**
   ```bash
   mysql -u orbifon_user -p orbifon
   ```

2. **Promote a user to moderator:**
   ```sql
   UPDATE users SET is_moderator = 1 WHERE id = 1;
   ```

3. **Verify they're a moderator:**
   ```bash
   # Try to access moderator endpoints
   curl http://localhost:8080/api/mod/reports/pending \
     -H "Authorization: Bearer <their_token>"
   ```

## Rate Limiting

The system enforces rate limits on:

- **Post creation**: 10 per hour
- **Comment creation**: 30 per hour
- **Voting**: 100 per hour
- **Reports**: 1 per hour

Exceeding limits returns: `429 Too Many Requests`

## Content Safety Checks

Automatic checks detect:

- Excessive capitalization (>70% caps)
- Profanity (configurable word list)
- Spam indicators:
  - Multiple links (>5)
  - Repeated characters (4+ same char in a row)

## Reputation System

Users earn/lose reputation based on:

- **+5** per post upvote received
- **+2** per comment upvote received
- **-20** per actioned report
- **-50** per suspension
- **+1** per month of account age

**Low Reputation Threshold:**
- Posts from users with <50 reputation are flagged for automatic review

## Audit Logging

All moderation actions are logged with:
- Actor ID (who took the action)
- Action type (what was done)
- Target type and ID (what was affected)
- Timestamps
- Additional details (JSON)

**Query audit logs:**
```sql
SELECT * FROM audit_logs 
WHERE action = 'user_suspended' 
ORDER BY created_at DESC LIMIT 10;
```

## Monitoring

### Check pending reports count:
```sql
SELECT COUNT(*) FROM reports WHERE status = 'pending';
```

### Check active suspensions:
```sql
SELECT u.username, s.reason, s.suspended_until 
FROM user_suspensions s
JOIN users u ON u.id = s.user_id
WHERE s.suspended_until > NOW() OR s.is_permanent = 1;
```

### Check rate limit abuse:
```sql
SELECT user_id, action, COUNT(*) as count
FROM rate_limit_events
WHERE created_at > DATE_SUB(NOW(), INTERVAL 1 HOUR)
GROUP BY user_id, action
HAVING count > 10;
```

### View user reputation scores:
```sql
SELECT username, reputation_score FROM users ORDER BY reputation_score DESC LIMIT 20;
```

## Best Practices

1. **Review reports promptly** - Respond to reports within 24-48 hours
2. **Document decisions** - Use the reason field to explain actions
3. **Be consistent** - Apply same standards across similar reports
4. **Escalate when needed** - Escalate serious violations to platform admins
5. **Appeal process** - Consider implementing user appeals for false positives
6. **Regular audits** - Review audit logs weekly to ensure moderator accountability

## Troubleshooting

### "Moderator access required" error
- Ensure user has `is_moderator = 1` in database
- Ensure user's JWT token is valid and not expired

### Reports not showing
- Check database connection
- Verify migration 0002_moderation.sql ran successfully
- Check for SQL errors in logs

### Rate limit not working
- Ensure rate_limit_events table exists
- Check that user_id and action parameters are correct
- Verify database timestamp is accurate
