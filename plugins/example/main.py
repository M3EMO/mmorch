"""Example mmorch plugin — authoring template (graft G11).

A contribution is a top-level function `fn(args, host)`:
  args  = the JSON dict from the invoke body's "args".
  host  = host("<ns>.<method>", params) calls a host service. Allowed ONLY if the
          manifest declares the <ns> capability AND the server policy
          (MMORCH_PLUGINS_ALLOW) grants it; otherwise it raises. Default policy = deny all.
Return any JSON-serializable value. The plugin runs in an isolated subprocess; print()
goes to the server log, never the protocol channel.
"""


def echo(args, host):
    """No host calls -> needs no capability."""
    text = str(args.get("text", ""))
    return {"reversed": text[::-1], "len": len(text)}


def summarize(args, host):
    """Uses caps llm + log: ask the host to run a model on our behalf."""
    text = str(args.get("text", ""))
    host("log.emit", {"msg": f"summarize {len(text)} chars"})       # cap "log"
    out = host("llm.call", {                                        # cap "llm"
        "model": args.get("model", "deepseek-chat"),
        "messages": [{"role": "user", "content": f"Summarize in one line:\n{text}"}],
    })
    return {"summary": out}
