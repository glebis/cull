# Vision Brainstorm — Raw Transcript
**Source:** Claude.ai chat "Accessible AI tool with MCP and CLI integration"
**Date:** 2026-05-10, 17:28–17:50 CET
**Format:** Voice dictation → Claude.ai conversation

---

## Message 1 (17:28)

So I wanna think about what is the AI first up. So for me, it should be fully accessible for agents via MCP or CLI, also fully accessible for people. A good UX, good standards. Also should be as little reliant on AI as possible and as much rely on predictable deterministic flows. It should fail gracefully. It shouldn't stop functioning when the Internet or AI is not available. It should allow to export data and actually allow access to this data, user's data via MCPs and other protocols, making really comfortable convenience to connect. And I'm building an app myself. It's an image viewer. It's for people who are generating lots of variants, and they need to view them, maybe annotate and share with others. Yes. I think... yeah. While I was telling you about it, I decided it should be easy for anyone to share it, everything from the using MCPs, using your cell deployment on Netlify or something like that.

## Message 2 (17:30)

Yeah. This would be anything, uh, we will give, like, really convenient access to agents, uh, to deploy whenever they want. Uh, we will have a couple of settings or prescriptions for agents and maybe non... even deterministic flows for some basic, um, providers, but that's about it. Uh, anyone can plug in anything. Uh, the app will just output the structured data, the files, plus some metadata. It should also be able to expose the data from the local server as well for read only purposes.

## Message 3 (17:31)

Yeah. I will already build the the MCP with thirty two tools, and I already tasked... tested it with code using this tool to demo me. It can open. It can help me select something. Yeah. I think I should also

## Message 4 (17:32)

Yeah. For hands off experience, I should support... yeah. I would put more data on the screen and maybe in the larger view so that larger font sizes so that it's... could be viewed from away. And, actually, agents should be able to control the UX. It should be able to output any kind of data, like, in the loop mode. We have this loop mode, like zoom in mode, where we only have one image. We should be output... able to output whether this image is selected, approved, or rejected. Whether it has rating. And, yes, we should be able to rate via MCP. And, yeah, that's it, I guess.

## Message 5 (17:32)

Yeah. Yeah. So the ideas that we come up together, we have a bunch of images.

## Message 6 (17:33)

Review them remotely or share them. We edit them. Uh, we reorganize them. So, yes, I can, uh, easily come from, uh, the grid, uh, to the Canvas. So, uh, where I can edit, uh, and make something larger, smaller when I can quickly crop actually everything. Everything should be croppable. And I can also maybe annotate and add comments and stuff like that.

## Message 7 (17:36)

I think it could be many things. Also, I would like to have flexibility. This is definitely privacy first local first thing. So, yes, annotations should be saved somewhere. We do support the sidecar, JSONs for images, which are GPT and my image generator actually produces already. And, yes, this is also something should be viewable. So when we have a prompt, it should be in the... when we are in the loop mode, and we have, like, not not, like, full screen without any items, but we have, like, some elements on, some information on. So we have stars, and we can also have an icon indication that there is a prompt. And we can... yes. From there there... right from there, we should be able to submit that prompt to to model to for for starters, let's say it's just passing the skill parameters, uh, or maybe just calling a deterministic, uh, CLI from the skill we have. Uh, and, yeah, that's that's pretty perfect. Yeah. We can use browser, uh, for that, or or or we can use, uh, things like, uh, API Yeah. The official API is pretty cool.

## Message 8 (17:36)

The default would be the official API. And, yes, I think we can do drafts, and we can do the full size image we already have, like, budget calculations and things like that.

## Message 9 (17:39)

Yeah. It say, uh, we'll need, like, something more dedicated from the editor. Uh, actually, I'm thinking, firstly, we should be able to submit, uh, two versions of the prompts to the same model and see the difference quickly or even four versions. Uh, we can do, like, whatever iterations, whatever number of iterations we want. Uh, that could be a parameter of up to, I don't know, hundreds. Uh, and we can generate, like, on the Canvas, we can generate the trees various styles or we can, uh, pop into any kind of trade and alter, like, just some parameters of the prompt. Uh, maybe the style or the color. So I don't know whether you poke or the elements or the full description, uh, of the interface content. So, yeah, it's mostly for, like, interface work, uh, or imagination. I don't know. Uh, also, I want this to be as modular as possible, maybe from the onset. Let's create a plug in system, and then let's implement, uh, all basic stuff through the model or plug in system, which is really, uh, like, it's... has really model or... and the, uh, full proof workflow, uh, with some built in validation and really concise and clear and laconic and really up to the point. Uh, evidence based, uh, kind of engineering practices, uh, not all of that.

## Message 10 (17:41)

Yeah. I think maybe let's build the course plugins too. I'm thinking about, uh, real hardware level plugins, you know, in the computer way. You can literally replace how you process some documents or how you do, like, the really basic stuff. Maybe similar to Linux. I'm not sure how how much, uh, access you have. And maybe there should be some, uh, limitations. Yes. Of course, then plugins become the huge liability as well, and anything can happen for plug ins. So a certain level of security should be there. But, yes, maybe all of the elements... currently, I don't think they are plug ins.

## Message 11 (17:42)

Okay. Yes. Actually, let's build the canvas, uh, first. Uh, whatever the architecture architecture is here right now, I don't wanna overdo it. I don't wanna spend too much, uh, talking on that, uh, but let's keep, uh, yeah. I will... I think I will need the list of requirements for such a system. Maybe in a few layers. And, also, I am thinking, can we apply DSPY to that, uh, the technology for building, uh, all deterministic flows using LLMs? Uh, I really like the the the... you actually have, like, self improvements loops that would make the workflows better and better than what they they run.

## Message 12 (17:46)

Okay. Okay. Okay. Okay. Yes. I think I wanna focus. Uh, I already have, like, basic processing... image processing, uh, imports, uh, various ways to view it. I need some basic editing. I need the canvas. I need some really smart, uh, so I have a few local models, like, image detection and yellow object detection. I think I can improve on that. I also... there's there's actually a huge direction. Uh, I have, uh, like, multi model embeddings, uh, from Google created for that, uh, and I wanna use that really smartly, uh, showing, like, vector spaces of images. I think it can be really cool interface. Uh, we are not, uh, there yet. And I also have, uh, all those additional models like object detection. Yeah. And, also, we can have people detection. I think there there was couple of open, uh, tools for that. And that's basically becoming, like, your cool... like, so many use cases, especially with the open architecture. I'm feeling I'm onto something big here. Uh, you can challenge me on that, but I do feel that building fast reliable, uh, AI first time MCP enabled tools, first CLI enabled, uh, agent agent aware, uh, being able out of the box outputs, uh, JSONs, and being a server, and supports in, uh, uh, protected, uh, network broadcasting, and lot lot of crazy stuff. Uh, it's a server, and it's a receiver as well. It should be able to connect to remote stream of in and the the commands. Yeah. So I pictured the, like, the remote editorial process. So we are editing someone's photo shoot like I was attending. So game exhibitions, workshops, and the most effective, the most cool part of my... of the job was when people would bring their photos. They would be shown, and they would select those photos he was selected, and I did them right in front of them. And what he did, this should be technically possible to do in my software. He only used the cropping, maybe some rotation in mid. Also... yeah. This definitely should be... we can... you should be able just to see... do this and that. And this is almost ready. We have already some... something like that. And what else?

## Message 13 (17:48)

Yes. Yes. Like, the shared state, uh, and there should be different roles. There should be editors. There should be observers. Maybe some... someone is a student, and he's just observing, uh, uh, the teacher be... using those tools. Maybe there are assistance. They can also use those tools. Uh, but, uh, I mean, if everyone is a re... an editor and they are, like, twenty or thirty people, like, my... I guess it might easily become a mess, and then there should be, like, moderation level on top of that. Yeah. And... yeah. Yeah. Yeah. I think it's... it could be all of that. It should be really flexible also.

## Message 14 (17:48)

The roles and capabilities should be configurable on the go, right, in the chat and the presentation mount.

## Message 15 (17:50)

Yeah. You know what? I think I will drop this conversation in the Claude Code, and I wanna see what it has to say because it has way more context than you. And I don't wanna be, uh, you know, speaking. But I have... that... like, our ideas are really great. Everything I've told right now is really cool. I wanna use it. Yes. So

## Message 16 (17:50)

I will stop here, I think.
