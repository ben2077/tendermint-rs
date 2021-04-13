# ADR 009: Transport agnostic Peer abstraction

## Changelog
* 2021-02-05: drafted

## Context

With the opportunity to design and implement the peer-to-peer stack from
scratch in the context of the Tendermint implementation in Rust, a lot of the
learnings of the shortcomings of the original Go implementation can be used to
prevent certain mistakes. Namely two:

* Leakage of physical concerns into the core domain
* Flexibility to adopt different wire protocols for transport of messages

For that, the first set of newly introduced concepts will attempt to be generic
over which transport is used to connect and converse with other peers. Given
strongly tailored abstract interfaces, concrete implementations will be easy to
spin up and plug into the machinery which lifts bytes from the wire into the
core domain and transports messages into the rest of the system.

## Decision

### Transport

Wraping the design is the `Transport`. Modelled with the properties of
a physical network endpoint in mind, which can be bound and stopped. It should
strongly correspond to the acquisition and lifecycle management of network
resources on the system.

``` rust
pub trait Transport {
    type Connection: Connection;
    type Endpoint: Endpoint<Connection = <Self as Transport>::Connection>;
    type Incoming: Iterator<Item = Result<<Self as Transport>::Connection>> + Send;

    fn bind(&self, bind_info: BindInfo) -> Result<(Self::Endpoint, Self::Incoming)>;
    fn shutdown(&self) -> Result<()>;
}
```

After the successful bind the caller holds an `Endpoint` as well as a stream of
incoming `Connection`s. Which is a standardised way to connect to new peers and
react to newly connected ones respectively.

``` rust
pub trait Endpoint: Send {
    type Connection;

    fn connect(&self, info: ConnectInfo) -> Result<Self::Connection>;
    fn listen_addrs(&self) -> Vec<SocketAddr>;
}
```

Centerpiece of the whole shebang is the `Connection`. It represents a connected
peer and provides the primitives to get data and send data from a peer. It is
designed with the outlook to support stream based transports down the road.
While being open to enable feature parity with current production installations
based on tendermint-go's `MConn`.

``` rust
pub trait Connection: Send {
    type Error: std::error::Error + Send + Sync + 'static;
    type Read: Read;
    type Write: Write;

    fn advertised_addrs(&self) -> Vec<SocketAddr>;
    fn close(&self) -> Result<()>;
    fn local_addr(&self) -> SocketAddr;
    fn open_bidirectional(
        &self,
        stream_id: &StreamId,
    ) -> Result<(Self::Read, Self::Write), Self::Error>;
    fn public_key(&self) -> PublicKey;
    fn remote_addr(&self) -> SocketAddr;
}
```

### Peer

Given a correct implementation of a `Transport` and its `Connection` newly
established ones will be wrapped with a `Peer`. Which is in charge of setting
up the correct streams on the `Connection` and multiplex messages - incoming
and outgoing alike - efficiently. It's also an attempt to enforce
correct-by-construction constraints on the state machine of the peer. To avoid
misuse or unexpected transitions. The only way to construct is, is from an
existing connection which gives the caller a connected peer. When invoking run
on that one a fully function peer is "returned". Therefore the states look
like: `Connected -> Running -> Stopped`.

``` rust
impl<Conn> Peer<Connected<Conn>>
where
    Conn: Connection,
{
    pub fn run(self, stream_ids: Vec<StreamId>) -> Result<Peer<Running<Conn>>> {
        // ...
    }

    fn stop(self) -> Result<Peer<Stopped>> {
        // ...
    }
}

impl<Conn> Peer<Running<Conn>>
where
    Conn: Connection,
{
    pub fn send(&self, message: message::Send) -> Result<()> {
        // ...
    }

    pub fn stop(self) -> Result<Peer<Stopped>> {
        // ...
    }
}
```

While sending messages is done through a method on a running peer, getting hold
of incoming messages can be achieved by draining the `Receiver` part of the
running state.


### Supervisor

The `Supervisor` is the main entry point to the p2p package giving higher-level
components access to a unified stream of peer events and messages as well as
the ability to control peer lifecycle (connect, disconnect, etc.).

``` rust
pub enum Command {
    Accept,
    Connect(SocketAddr),
    Disconnect(node::Id),
    Msg(node::Id, message::Send),
}

pub enum Event {
    Connected(node::Id, Direction),
    Disconnected(node::Id, Report),
    Message(node::Id, message::Receive),
    Upgraded(node::Id),
    UpgradeFailed(node::Id, Report),
}

impl Supervisor {
    pub fn run<T>(transport: T) -> Result<Self>
    where
        T: transport::Transport + Send + 'static,
    {
        // ...
    }

    pub fn recv(&self) -> Result<Event> {
        // ...
    }

    pub fn command(&self, cmd: Command) -> Result<()> {
        // ...
    }
}
```

## Status

Proposed

## Consequences

### Positive

* Unified way to bootstrap and integrate transports
* Potential for connecting different wire transports in the same process
* Rest of the domain is simply concerned with `node::Id`s as identity

### Negative

* Significant set of abstractions need to be satisfied for a new transport
  implementation
* Non-stream based transports need to be fitted into this model

### Neutral

## Open Questions

* Should serialization be transport specific to allow for optimisations on the
  wire per transport instead of expecting the same byte layout no matter which
  wire protocol in use?

## References

* [rfc: add P2P stream proposal](https://github.com/tendermint/spec/pull/227)
* [P2P Refactor](https://github.com/tendermint/tendermint/issues/2067)
* [p2p: support multiple transports](https://github.com/tendermint/tendermint/issues/5587)