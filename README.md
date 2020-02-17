# A TUI chat client for all the things

I really dislike how much my chromebook lags while trying to use Slack, Discord, and other messaging services. This is my attempt to bring a bunch of messaging services together with a fast interface. That's currently a TUI because GUI is hard and I mostly want to send and receive text.

![Build Status](https://circleci.com/gh/saethlin/omnichat.svg?style=shield&circle-token=:circle-token)

![omnichat_slack](omni_small.png)


A valid config file which must be placed at `$HOME/.omnichat.toml` looks like this:
```
[[slack]]
token = "slack_user_token_here"

[[slack]]
token = "slack_user_token_for_another_server"
```
To get your slack token for a server, you'll need to visit https://api.slack.com/legacy/custom-integrations/legacy-tokens

Working on: 
## Slack
* More commands like /join and /leave 
* Need a plan for threads
* Default emoji autocomplete
* Handle disconnect+reconnect

## Discord
* Working out how to request the unread cursor
* Mentions support

## UI
* Better /url regex
* Support for edits and deletions
