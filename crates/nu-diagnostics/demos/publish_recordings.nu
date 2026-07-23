#!/usr/bin/env nu

const RECORDINGS_BOOKMARK = "recordings"
const COMMIT_MESSAGE = "docs: regenerate recordings"

def main [recordings: path, remote_url: string] {
    let recorded_scenes = glob ($recordings | path expand | path join "*.webp")
    if ($recorded_scenes | is-empty) {
        error make { msg: $"no recordings in ($recordings)" }
    }
    let staging_directory = mktemp --directory --suffix "-nu-diagnostics-recordings"
    $recorded_scenes | each {|scene| cp --no-symlink $scene $staging_directory }
    cd $staging_directory
    ^chmod -R u+w .
    ^jj git init
    ^jj git remote add origin $remote_url
    ^jj git fetch --remote origin --branch $RECORDINGS_BOOKMARK
    ^jj describe --message $COMMIT_MESSAGE
    let remote_exists = (
        ^jj bookmark list --all-remotes --template 'name ++ "@" ++ remote ++ "\n"'
        | lines
        | any {|bookmark| $bookmark == $"($RECORDINGS_BOOKMARK)@origin" }
    )
    if $remote_exists {
        let changed = ^jj diff --summary --from $"($RECORDINGS_BOOKMARK)@origin" --to "@" | str trim
        if ($changed | is-empty) {
            print "recordings are unchanged, nothing to publish"
            return
        }
        ^jj bookmark track $"($RECORDINGS_BOOKMARK)@origin"
        ^jj bookmark set $RECORDINGS_BOOKMARK --revision "@" --allow-backwards
    } else {
        ^jj bookmark create $RECORDINGS_BOOKMARK --revision "@"
    }
    ^jj git push --bookmark $RECORDINGS_BOOKMARK
}
