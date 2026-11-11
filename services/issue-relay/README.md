# anyvector-issues: bug reports and ideas without a GitHub account

AnyVector's *Help › Report a Bug* and *Help › Submit Ideas and Feedback* send their form here. This Cloudflare Worker:
- checks the report
- allows at most 2 reports a minute from one address
- creates the issue on `toim22/AnyVector` as the **AnyVector Reports** GitHub App, labeled `bug` or `enhancement`

The person reporting needs no GitHub account. No key ships inside AnyVector: the app's private key lives only in the
Worker's secrets.

When the Worker can't be reached, AnyVector falls back to opening GitHub's new-issue page in the browser, filled in.
That path needs a GitHub account.

It is the same service as AnyCapture's (`services/issue-relay` in that repository), with its own name, GitHub App and
repository.

## Files

- `src/index.ts`: `POST /issues` (JSON) → `201 {"url": …}`. It answers 400 when it refuses a report, 413 when the
  report is too large, 429 when there were too many, and 502 when GitHub failed. `GET /` answers `ok`.
- `src/report.ts`: the checks (kind, required title and description, lengths, version, program) and the issue's
  text. `@name` mentions and "fixes #1" are defused, so a report can't ping people or close issues.
- `src/github.ts`: the GitHub App sign-in and the issue creation. A JWT signed with the private key gets a one-hour
  installation token, which is kept while valid.
- `test.ts`: offline tests. Run them with `npm test`; every deploy runs them first.

## Set up (once)

### 1. Create the GitHub App

1. Open <https://github.com/settings/apps/new> (signed in as `toim22`).
2. **GitHub App name**: `AnyVector Reports`. Any free name works; it shows on the issues as `<name>[bot]`.
3. **Homepage URL**: `https://github.com/toim22/AnyVector`.
4. **Webhook**: untick **Active** (the app receives nothing).
5. **Permissions › Repository permissions › Issues**: **Read and write**. Leave everything else as it is
   (*Metadata: Read-only* is added by itself).
6. **Where can this GitHub App be installed?** **Only on this account**.
7. **Create GitHub App**.

### 2. Get its ID and private key

1. On the app's page, copy the **App ID** (a number, under *About*).
2. At the bottom, **Private keys › Generate a private key**. GitHub downloads a `.pem` file.
3. Save it in this folder as `github-app.pem`. It never goes into Git (`.gitignore`).

### 3. Install it on the repository

1. In the app's settings, left menu, **Install App** → **Install** next to `toim22`.
2. **Only select repositories** → `AnyVector` → **Install**.

### 4. Publish the Worker

In this folder (`services/issue-relay`):

1. Copy `secrets.example.env` to `secrets.env` and put the App ID after `GITHUB_APP_ID=`.
2. Run:

```bash
npm.cmd install
```

```bash
npm.cmd run first-deploy
```

`first-deploy` does three things:
- runs the tests
- adds the private key to `secrets.env` (`secrets.mjs`)
- publishes with `cf deploy --secrets-file secrets.env`

If `cf` isn't signed in to Cloudflare yet, it asks you to sign in in the browser. At the end it prints the address,
`https://anyvector-issues.<your subdomain>.workers.dev`.

3. Check it: opening that address in the browser shows `ok`.
4. Delete `secrets.env` and `github-app.pem`, or keep the `.pem` somewhere safe outside the repository. The keys are
   stored at Cloudflare now.

### 5. Point AnyVector at it

`RELAY_URL` in `crates/anyvector/src/bugreport.rs` holds the address. To try a Worker somewhere else without
rebuilding, set the environment variable `ANYVECTOR_ISSUE_RELAY` to its address before starting AnyVector.

## Every day

- **Changed the code?** Run `npm.cmd run deploy` (tests first).
- **Logs:** Cloudflare dashboard › Workers & Pages › `anyvector-issues` › Logs. They show each created issue and every
  GitHub error.
- **A new private key** (the old one leaked or was deleted): generate it on the app's page and save it as
  `github-app.pem`. Put the ID in `secrets.env` again and run `npm.cmd run first-deploy`.
- **Spam:** close or delete the issue on GitHub. To stop all reports at once, uninstall the app from the repository
  (GitHub › Settings › Applications › AnyVector Reports › Configure › Uninstall). AnyVector then opens the browser
  form instead.

## Cost

Free. The Workers free plan covers 100,000 requests a day, and the rate limiting binding is included.
