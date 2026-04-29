# Skills

Skills are optional task guides that Personal Agent can use during a conversation. A skill usually describes a specialized workflow, format, or tool-use pattern, such as working with a particular document type or following a repeatable research process.

Skills do not replace the model profile you select for chat. They add focused instructions that help the assistant respond consistently when a task matches the skill.

## Why use skills?

Use skills when you want Personal Agent to follow a known approach for a recurring task. For example, a skill can help the assistant:

- Follow a preferred report or briefing format.
- Work with a specific file type.
- Apply a checklist before producing an answer.
- Remember a repeatable workflow without retyping the instructions in every chat.

## Where to manage skills

Open **Settings**, then choose the **Skills** section.

From there you can:

- Review discovered skills.
- Enable or disable individual skills.
- Refresh the skills list after adding or editing skill files.
- Open the default skills folder.
- Add extra watched directories that contain skills.

Personal Agent discovers bundled skills, skills in the default user skills folder, and skills from any watched directories you add.

## How skills affect assistant behavior

When a skill is enabled, Personal Agent may tell the selected model that the skill is available. The assistant can then activate the skill when it seems relevant to your request.

Important details:

- Disabled skills are not offered to the assistant.
- Enabling a skill does not force every response to use it.
- A skill can influence the assistant's instructions, planning, and tool choices for matching tasks.
- If tool approval is enabled, Personal Agent may ask before activating a skill or using tools suggested by that skill.

You can always ask the assistant directly to use or avoid a specific skill in the current conversation.

## Adding your own skills

The simplest way to add personal skills is to place skill files in the default skills folder shown in Settings. You can also add a watched directory if you keep skills somewhere else.

After adding or editing skills, click **Refresh Skills** in Settings so Personal Agent rescans the folders.

## Safety notes

Skills can change how the assistant approaches a task, so only enable skills you understand and trust.

Before enabling a skill, consider:

- What instructions it gives the assistant.
- Whether it encourages use of files, network services, or MCP tools.
- Whether it matches the kind of work you want Personal Agent to do.

Use the **Tool Approval** settings if you want Personal Agent to ask before skill activation or related tool use.

## Troubleshooting

### A skill does not appear

- Click **Refresh Skills** in Settings.
- Confirm the skill file is in the default skills folder or a watched directory.
- Confirm the watched directory still exists and can be read by your user account.
- Restart Personal Agent if the list still does not update.

### A skill appears but is not used

- Confirm the skill is enabled.
- Ask the assistant explicitly to use the skill for the task.
- Check whether your request actually matches the skill's purpose.

### A watched directory fails to add

- Confirm the path is not empty.
- Confirm your user account can create or read the directory.
- Use an absolute path if a relative path does not resolve as expected.

### Skill activation asks for approval

This is expected when tool approval settings require confirmation. Approve the activation only if you trust the skill and it is relevant to the task.
