# slack-rs-api

This is a fork of the original slack-rs-api project with different goals and a different approach motivated by my own use of the master branch.

Unlike the master branch, I don't think that scraping the Slack API documentation is a viable way to produce. The Slack API documentation is far from complete, and there are constraints and meaning in its implied types that cannot be trivially extracted.

In response to a lot of deserialization failures, the master branch also made nearly every field on a deserialized struct an `Option`. I don't think this is a fix. With the default serde Derive implementation, a struct whose fields are all `Option`s will successfuly deserialize from any input. Neither missing fields nor fields present in the serialization format (JSON) but not in the struct definition are errors. The "everything is optional" approach also force the user to write either very panicky code that unwraps/expects everything or makes the user pick some sort of fallback for potentially missing values, often unnecessarily.


# Goals
* Provide structs and serde implementations to provide type safety around the Slack web APIs
* Avoid tying the use of this crate to any piece of the web ecosystem (hyper, reqwest, openssl)
* If you know the Slack web APIs, the layout of this crate should be obvious

# Non-Goals
* This is not a "for humans" crate. We primarily provide type safety and will try to provide convienence where possible, but if you're looking for a project that lets you whip up a slack bot in a few lines this is not it.
* Parts of the Slack API are messy, and so this will be too. I'm sorry?

