#include "scenario.hpp"

#include <algorithm>
#include <cstdio>

namespace desk::sim {
namespace {

using domain::Agent;
using domain::SessionState;

struct Seed {
  std::uint32_t id;
  Agent agent;
  SessionState state;
  const char* name;
  const char* detail;
  int secondsAgo;
};

constexpr Seed kSeeds[] = {
    {1, Agent::Claude, SessionState::NeedsYou, "servarium-05", "perm", 42},
    {2, Agent::Codex, SessionState::NeedsYou, "Introduce gated SFTP access", "?", 190},
    {3, Agent::Claude, SessionState::Done, "documents-b9", "", 720},
    {4, Agent::Claude, SessionState::Working, "desksessions-d9", "Bash", 65},
    {5, Agent::Codex, SessionState::Working, "Check prod crosspost thumbnails", "", 500},
};

const char* const kNewNames[] = {"maecenas-4e", "Cabinet new look feature flag", "landing-copy-v2",
                                 "Миграция базы данных", "auth-evidence-review"};

}  // namespace

Scenario::Scenario(Instant start) : stepAt_(start) {
  for (const Seed& seed : kSeeds) {
    Live live;
    live.session.id = domain::SessionId{seed.id};
    live.session.agent = seed.agent;
    live.session.state = seed.state;
    live.session.name = seed.name;
    live.session.detail = seed.detail;
    live.since = start - Millis{seed.secondsAgo * 1000};
    sessions_.push_back(live);
  }
  if (Live* codex = find(5)) {
    codex->session.subagents = 3;
  }
}

bool Scenario::advance(Instant now) {
  if (auto_ && now - stepAt_ >= Millis{3'500}) {
    stepAt_ = now;
    switch (step_++ % 8) {
      case 0:
        addSession(now);
        break;
      case 1:
        setState(4, SessionState::NeedsYou, "perm", now);
        break;
      case 2:
        setState(1, SessionState::Working, "Edit", now);
        break;
      case 3:
        setState(4, SessionState::Working, "Read", now);
        break;
      case 4:
        setState(1, SessionState::Done, "", now);
        break;
      case 5:
        removeLast();
        break;
      case 6:
        setState(1, SessionState::NeedsYou, "perm", now);
        break;
      case 7:
        setState(3, SessionState::Done, "", now);
        break;
    }
  }
  const bool changed = dirty_;
  dirty_ = false;
  return changed;
}

domain::Snapshot Scenario::snapshot(Instant now) const {
  domain::Snapshot snapshot;
  for (const Live& live : sessions_) {
    if (snapshot.sessions.full()) {
      break;
    }
    domain::Session s = live.session;
    s.inState = now - live.since;
    snapshot.sessions.push_back(s);
  }
  return snapshot;
}

void Scenario::send(const domain::Command& command) {
  const bool dismiss = command.kind == domain::Command::Kind::Dismiss;
  std::printf("[mac] %s session %u\n", dismiss ? "dismiss" : "jump to",
              static_cast<unsigned>(command.session.value));
  if (dismiss) {
    std::erase_if(sessions_, [&](const Live& l) { return l.session.id == command.session; });
    dirty_ = true;
  }
}

void Scenario::addSession(Instant now) {
  Live live;
  live.session.id = domain::SessionId{nextId_};
  live.session.agent = nextId_ % 2 == 0 ? Agent::Claude : Agent::Codex;
  live.session.state = SessionState::Working;
  live.session.name = kNewNames[nextId_ % std::size(kNewNames)];
  live.session.detail = "Grep";
  live.since = now;
  ++nextId_;
  sessions_.push_back(live);
  dirty_ = true;
}

void Scenario::removeLast() {
  const auto it = std::find_if(sessions_.rbegin(), sessions_.rend(),
                               [](const Live& l) { return l.session.id.value >= 100; });
  if (it != sessions_.rend()) {
    sessions_.erase(std::next(it).base());
    dirty_ = true;
  }
}

void Scenario::escalateFirstWorking(Instant now) {
  for (Live& live : sessions_) {
    if (live.session.state == SessionState::Working) {
      setState(live.session.id.value, SessionState::NeedsYou, "perm", now);
      return;
    }
  }
}

void Scenario::toggleEmpty() {
  if (sessions_.empty()) {
    sessions_.swap(stash_);
  } else {
    stash_ = sessions_;
    sessions_.clear();
  }
  dirty_ = true;
}

Scenario::Live* Scenario::find(std::uint32_t id) {
  const auto it = std::find_if(sessions_.begin(), sessions_.end(),
                               [id](const Live& l) { return l.session.id.value == id; });
  return it == sessions_.end() ? nullptr : &*it;
}

void Scenario::setState(std::uint32_t id, SessionState state, const char* detail, Instant now) {
  Live* live = find(id);
  if (live == nullptr) {
    // Dismissed earlier: bring it back as a fresh session.
    sessions_.push_back(Live{});
    live = &sessions_.back();
    live->session.id = domain::SessionId{id};
    live->session.name = "servarium-05";
  }
  if (live->session.state != state) {
    live->since = now;
  }
  live->session.state = state;
  live->session.detail = detail;
  dirty_ = true;
}

}  // namespace desk::sim
