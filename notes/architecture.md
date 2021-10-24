# Architecture

## Goals

- Strictness/Correctness
- Robustness
- Minimalism
- Cross-plaftorm

## Components

- One actor per peer connection
  * Split into 2 parts: the stream of frames (reading, with `Actor::add_stream`) and writing frames 
  * Sends/checks heartbeats
- One actor per file (one file for the first working version) with exclusive access to the file on the filesystem
  * Checksums (periodically?) the pieces and asks the pieces actor for a piece if it was invalidated
  * How to handle concurrent instances of the program? Lock files? Allow it?
- One actor for the pieces
  * The actual logic of requesting pieces, choking etc is there
  * Should it spawn the peer actors? And re-fetch the peer list from the tracker periodically?
  * Need random sampling or scoring to choose peers to talk to
- Need timeouts/retries/backoff everywhere when doing I/O
- What do to when an actor's mailbox is full? Wait?
- Fuzzing?
- Sharing pieces is not yet considered
  * A peer can forward a block request to the pieces actor which then asks it if we have it from the file actor?

## Scope

*Still considering if those belong to the scope*

- Limits? Open connections, memory, connection throttling?
- What is the UI? CLI, term UI, local web UI ... ?
- Where to see logs? Log file, UI?
- Tracker implementation
- Bittorrent protocol: v1, magnet, v2, extensions
- Fetch torrent file from a URL

## Testing

- Spawn two instances with 2 different files and let them share both until they converge
- Bad actors?
