# User Auth TODO

현재 구현된 범위:

- `POST /signup`
- `POST /login`
- `GET /me`
- `POST /admin/add-user`

남은 작업:

- Add `PUT /me/password`
  - Require current password verification
  - Enforce password policy and bcrypt re-hash

- Add `DELETE /me`
  - Define deletion policy for owned `posts`, `images`, `budget_config`, and `spending_records`
  - Decide whether admin accounts can self-delete

- Add refresh-token flow
  - Implement `POST /refresh`
  - Decide refresh token rotation and invalidation policy

- Add logout strategy
  - Decide whether logout is purely client-side or backed by server-side refresh token invalidation

- Consider renaming login/signup identifiers
  - Current login uses `user_id` field but semantically expects email
  - Decide whether to keep compatibility or move to explicit `email`

- Strengthen auth validation
  - Normalize email input
  - Standardize auth error responses
  - Revisit password policy beyond minimum length

- Add admin user-management APIs
  - `GET /admin/users`
  - `GET /admin/users/:user_id`
  - `PUT /admin/users/:user_id`
  - `DELETE /admin/users/:user_id`

- Expand auth/account tests
  - `/me` unauthorized access
  - Admin login success path
  - Password change invalidates old password
  - Deleted account cannot log in
  - `/me` response for both `user` and `admin`
