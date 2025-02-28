## 1. Create Knowledge-Base

First, let's say there're text files explaining ai. We'll build a knowledge-base with from the text files. The directory should look like below.

```
ai_tutorials/
 ├ ai_tutorial_1.txt
 ├ ai_tutorial_2.txt
 ├ ai_tutorial_3.txt
 ╰ ... and many more txt files
```

Run `cd ai_tutorial; rag init`. You'll see a new directory created like below.

```
ai_tutorials/
 ├ .ragit/
 │  ├ chunks/
 │  ├ configs/
 │  ├ prompts/
 │  ╰ index.json
 ├ ai_tutorial_1.txt
 ├ ai_tutorial_2.txt
 ├ ai_tutorial_3.txt
 ╰ ... and many more txt files
```

`.ragit/` is like `.git/` of git repositories. It saves metadata and chunks. After `rag init`, the knowledge-base is empty. You have to add files to the staging using `rag add` command.

Run `rag add --all`. Now you're ready to build the knowledge-base. Run `rag build` to start the work. The default model is `llama3.3-70b-groq` and you need `GROQ_API_KEY` to run. If you want to run gpt-4o-mini, run `rag config --set model gpt-4o-mini`. You can see the list of the models using `rag ls-models`. You can also add models manually to `.ragit/models.json`.

```
elapsed time: 00:33
staged files: 15, processed files: 13
remaining chunks (approx): 27
committed chunks: 1
currently processing files: `ai_tutorial_14.txt`, `ai_tutorial_21.txt`
buffered files: 8, buffered chunks: 8
model: gpt-4o-mini
input tokens: 14081 (0.001$), output tokens: 1327 (0.000$)
```

`rag build` takes very long time and money (if you're using a proprietary api). It creates chunks and add title and summary to each chunk, using AI.

You can press Ctrl+C to pause the process. You can resume from where you left off by running `rag build` again. (more on [a dedicated document](./commands/build.txt))

```
ai_tutorials/
 ├ .ragit/
 │  ├ chunks/
 │  │  ╰ ... a lot of files ...
 │  ├ configs/
 │  ├ prompts/
 │  ╰ index.json
 ├ ai_tutorial_1.txt
 ├ ai_tutorial_2.txt
 ├ ai_tutorial_3.txt
 ╰ ... and many more txt files
```

After it's built, you'll see many data files in the `.ragit/` directory. You can ask queries on the knowledge-base now.

NOTE: You can ask queries on an incomplete knowledge-base, too.

## 2. (Optional) Clone Knowledge-Bases from web

This is the key part. You can download knowledge-bases from the internet and extend your knowledge-base with those. You can also share your knowledge-base with others.

First, let's make a fresh directory. Run `mkdir playground; cd playground`.

```
playground
```

Before downloading knowledge-bases, we have to init a rag index. Run `rag init`.

```
playground
 ╰ .ragit
    ├ chunks/
    ├ configs/
    ├ prompts/
    ╰ index.json
```

You'll see an empty rag index. Now we have to download knowledge-bases from the web. I have uploaded a few sample knowledge-bases for you. You can `rag clone` them, like `rag clone http://ragit.baehyunsol.com/sample/git`

- [git](http://ragit.baehyunsol.com/sample/git)
- [ragit](http://ragit.baehyunsol.com/sample/ragit)
- [rustc-dev-guide](http://ragit.baehyunsol.com/sample/rustc)

Let's clone all of them. Run `rag clone http://ragit.baehyunsol.com/sample/git; rag clone http://ragit.baehyunsol.com/sample/ragit; rag clone http://ragit.baehyunsol.com/sample/rustc;`

```
playground
 ├ .ragit
 │  ├ chunks/
 │  ├ configs/
 │  ├ prompts/
 │  ╰ index.json
 ├ git
 │  ╰ .ragit
 │     ├ chunks/
 │     ├ configs/
 │     ├ prompts/
 │     ╰ index.json
 ├ ragit
 │  ╰ .ragit
 │     ├ chunks/
 │     ├ configs/
 │     ├ prompts/
 │     ╰ index.json
 ╰ rustc
    ╰ .ragit
       ├ chunks/
       ├ configs/
       ├ prompts/
       ╰ index.json
```

Now we have 1 empty knowledge-base and 3 complete knowledge-bases in the playground. We're gonna use the empty knowledge-base as the main one. Let's extend the empty one. Run `rag merge ./git --prefix=git`, `rag merge ./ragit --prefix=ragit` and `rag merge ./rustc --prefix=rustc`.

## 3. Change Configs

Before asking a question or building a knowledge-base, you may want to change configurations. Configurations are very important because most commands cost money and you can optimize it with proper configurations.

### Per-Repository Configuration

Run `rag config --get model`. You'll see which model is used to answer your queries and build a knowledge-base.

Let's say you have free credits for Anthropic. By running `rag config --set model claude-3.5-sonnet`, you can change your default model.

Run `rag config --get-all` to see all the keys and values.

### Global Configuration

If you want to set default configurations for all your repositories, you can create configuration files in `~/.config/ragit/`:

- `~/.config/ragit/api.json` - For API configuration (model, timeout, etc.)
- `~/.config/ragit/build.json` - For build configuration (chunk size, etc.)
- `~/.config/ragit/query.json` - For query configuration (max titles, etc.)

These files can contain just the specific fields you want to override - you don't need to include all configuration options. For example, if you only want to set a default model, your `~/.config/ragit/api.json` could be as simple as:

```json
{
  "model": "claude-3.5-sonnet"
}
```

When you run `rag init` to create a new repository, these global configurations will be used as defaults. This is especially useful if you always want to use a specific model or have specific build parameters.

## 4. Build an inverted-index

[Inverted-index](https://en.wikipedia.org/wiki/Inverted_index) is a special kind of data structure that makes full-text searching much faster. By running `rag ii-build`, you can build an inverted index. Once you're done, run `rag ii-status` and see if it says "complete". If so, you're good to go. Text retrieval will get much faster.

## 5. Ask questions on a Knowledge-Base

Asking query is straight forward: `rag query "Tell me how the rust compiler uses git"`

If you want an interactive chat, run `rag query --interactive`.
