// The git fast-import streams of make-perf-repos.ps1's synthetic repositories, and the changes it
// makes to the wide tree's working folder. Compiled by the script with Add-Type: generating a
// million commits in PowerShell itself would take far longer.
//
// Everything is deterministic: names, dates, contents and the random bytes of binary files come
// from the commit or file number (and a fixed seed), so two builds at one scale are alike.
using System;
using System.Collections.Generic;
using System.IO;
using System.Security.Cryptography;
using System.Text;

namespace GlowflyPerfRepos
{
    public sealed class FastImport
    {
        static readonly string[] People = {
            "Ada Lovelace", "Alan Turing", "Grace Hopper", "Edsger Dijkstra", "Barbara Liskov",
            "Donald Knuth", "Frances Allen", "Ken Thompson", "Margaret Hamilton", "Dennis Ritchie",
            "Jürgen Müller", "Radia Perlman", "John Backus", "Sophie Wilson", "Niklaus Wirth",
            "Hedy Lamarr", "Tony Hoare", "Karen Spärck Jones", "Leslie Lamport", "Mary Kenneth Keller",
        };
        static readonly string[] Subjects = {
            "Fix the parser's handling of empty lines", "Speed up the index walk",
            "Rename the helper for clarity", "Add a test for the über-long path case",
            "Document the config options", "Refactor the tokenizer", "Handle a missing remote",
            "Tidy up whitespace", "Update the changelog", "Guard against a null entry",
        };
        readonly Stream output;
        readonly Dictionary<string, long> files = new Dictionary<string, long>();
        long mark;
        long clock = 1_000_000_000;

        public FastImport(Stream output) { this.output = output; }

        public void Raw(string text)
        {
            var bytes = Encoding.UTF8.GetBytes(text);
            output.Write(bytes, 0, bytes.Length);
        }

        public void Data(string text)
        {
            var bytes = Encoding.UTF8.GetBytes(text);
            Raw("data " + bytes.Length + "\n");
            output.Write(bytes, 0, bytes.Length);
            Raw("\n");
        }

        // `length` bytes from a generator seeded with `seed`, written in pieces.
        public void RandomData(long length, int seed)
        {
            Raw("data " + length + "\n");
            var random = new Random(seed);
            var buffer = new byte[1 << 16];
            for (long left = length; left > 0; left -= buffer.Length)
            {
                random.NextBytes(buffer);
                output.Write(buffer, 0, (int)Math.Min(left, buffer.Length));
            }
            Raw("\n");
        }

        static string Ident(long n, long when)
        {
            var name = People[n % People.Length];
            var email = name.ToLowerInvariant().Replace(' ', '.') + "@example.com";
            return name + " <" + email + "> " + when + " +0000";
        }

        static string Message(long n)
        {
            var subject = Subjects[n % Subjects.Length];
            return subject + " (" + n + ")\n\nChange number " + n + " of the synthetic history.\n";
        }

        // Starts commit `n` on `branch` and returns its mark. The caller then writes its file
        // changes (Modify, Delete). `from` and `merges` are marks; 0 for none.
        public long Commit(string branch, long n, long from, IList<long> merges, string message = null)
        {
            clock += 97;
            mark++;
            Raw("commit " + branch + "\nmark :" + mark + "\n");
            Raw("author " + Ident(n, clock) + "\ncommitter " + Ident(n + 3, clock) + "\n");
            Data(message ?? Message(n));
            if (from > 0) Raw("from :" + from + "\n");
            if (merges != null) foreach (var m in merges) Raw("merge :" + m + "\n");
            return mark;
        }

        public void Modify(string path, string content, string mode = "100644")
        {
            Raw("M " + mode + " inline " + path + "\n");
            Data(content);
        }

        public void Delete(string path) { Raw("D " + path + "\n"); }

        public void Reset(string reference, long from) { Raw("reset " + reference + "\nfrom :" + from + "\n\n"); }

        public void Tag(string name, long from, long n)
        {
            clock += 1;
            Raw("tag " + name + "\nfrom :" + from + "\ntagger " + Ident(n, clock) + "\n");
            Data("Release " + name + "\n");
        }

        // A file's next revision: its path and a line naming the revision.
        string Revise(string path)
        {
            long r;
            files.TryGetValue(path, out r);
            files[path] = ++r;
            return path + " revision " + r + "\n";
        }

        // `commits` commits in a line on main, each changing one of `fileCount` files.
        public static void Linear(Stream s, long commits, int fileCount)
        {
            var f = new FastImport(s);
            for (long i = 0; i < commits; i++)
            {
                f.Commit("refs/heads/main", i, 0, null);
                var file = "src/m" + (i % 40).ToString("00") + "/file" + (i % fileCount).ToString("0000") + ".txt";
                f.Modify(file, f.Revise(file));
            }
        }

        // `commits` commits on main, `tags` tags on them (every tenth annotated) and `branches`
        // branches in folders, every fourth with a commit of its own.
        public static void Refs(Stream s, long commits, long tags, long branches)
        {
            var f = new FastImport(s);
            var marks = new long[commits];
            for (long i = 0; i < commits; i++)
            {
                marks[i] = f.Commit("refs/heads/main", i, 0, null);
                var file = "lib/part" + (i % 100).ToString("000") + ".txt";
                f.Modify(file, f.Revise(file));
            }
            for (long t = 0; t < tags; t++)
            {
                var name = "v" + (t / 10000) + "." + (t / 100 % 100) + "." + (t % 100);
                var at = marks[t % commits];
                if (t % 10 == 0) f.Tag(name, at, t); else f.Reset("refs/tags/" + name, at);
            }
            for (long b = 0; b < branches; b++)
            {
                var name = "refs/heads/team-" + (b % 50).ToString("00") + "/topic-" + b;
                var at = marks[b * 5 % commits];
                if (b % 4 == 0)
                {
                    f.Commit(name, commits + b, at, null);
                    f.Modify("topics/" + b + ".txt", "work on topic " + b + "\n");
                }
                else f.Reset(name, at);
            }
        }

        // The wide tree's file `i`: 100 files a folder, 100 folders a top-level folder.
        public static string WidePath(long i)
        {
            return "d" + (i / 10000).ToString("000") + "/s" + (i / 100 % 100).ToString("00") + "/f" + (i % 100).ToString("00") + ".txt";
        }

        // One commit adding `fileCount` files, then one changing every hundredth.
        public static void WideTree(Stream s, long fileCount)
        {
            var f = new FastImport(s);
            f.Commit("refs/heads/main", 0, 0, null, "Add the whole tree\n");
            for (long i = 0; i < fileCount; i++) f.Modify(WidePath(i), "wide tree file " + i + "\n");
            f.Commit("refs/heads/main", 1, 0, null, "Change every hundredth file\n");
            for (long i = 0; i < fileCount; i += 100) f.Modify(WidePath(i), "wide tree file " + i + ", changed\n");
        }

        // Many changes in the checked-out wide tree: every 50th file changed, every 199th
        // deleted, and one new (untracked) file for every hundred.
        public static void ChangeWideTree(string root, long fileCount)
        {
            var utf8 = new UTF8Encoding(false);
            for (long i = 0; i < fileCount; i++)
            {
                var path = Path.Combine(root, WidePath(i));
                if (i % 50 == 0) File.AppendAllText(path, "changed in the working tree\n", utf8);
                else if (i % 199 == 0) File.Delete(path);
            }
            for (long i = 0; i < fileCount / 100 + 1; i++)
            {
                var path = Path.Combine(root, "new", "n" + (i / 100).ToString("000"), "added" + i + ".txt");
                Directory.CreateDirectory(Path.GetDirectoryName(path));
                File.WriteAllText(path, "untracked file " + i + "\n", utf8);
            }
        }

        // `lanes` long-lived branches, each taking a commit a round and merged into main by one
        // octopus merge a round (`lanes` + 1 parents), and the last round left unmerged; then
        // `topics` short topic branches in batches of 100, all forked from one main commit and
        // merged back one by one, so up to 100 lanes run side by side.
        public static void Octopus(Stream s, long lanes, long rounds, long topics)
        {
            var f = new FastImport(s);
            long n = 0;
            var main = f.Commit("refs/heads/main", n++, 0, null, "Start the history\n");
            f.Modify("README.txt", "octopus history\n");
            var tips = new long[lanes];
            for (long l = 0; l < lanes; l++) tips[l] = main;
            for (long r = 0; r <= rounds; r++)
            {
                for (long l = 0; l < lanes; l++)
                {
                    var lane = "refs/heads/lane-" + l.ToString("000");
                    tips[l] = f.Commit(lane, n++, tips[l], null);
                    var file = "lanes/" + l + ".txt";
                    f.Modify(file, f.Revise(file));
                }
                if (r == rounds) break;
                main = f.Commit("refs/heads/main", n++, main, tips, "Merge all " + lanes + " lanes, round " + r + "\n");
            }
            for (long t = 0; t < topics; t += 100)
            {
                var fork = main;
                var batch = new List<long>();
                for (long b = t; b < Math.Min(t + 100, topics); b++)
                {
                    long tip = fork;
                    for (long c = 0; c <= b % 3; c++)
                    {
                        tip = f.Commit("refs/heads/topic/" + b, n++, tip, null);
                        var file = "topics/" + b + ".txt";
                        f.Modify(file, f.Revise(file));
                    }
                    batch.Add(tip);
                }
                foreach (var tip in batch)
                    main = f.Commit("refs/heads/main", n++, main, new[] { tip }, "Merge a topic branch\n");
            }
        }

        // A huge text file, a long single-line file, a large binary in three revisions and many
        // medium binaries. Sizes are in bytes.
        public static void BigFiles(Stream s, long textBytes, long binaryBytes, long mediumCount, long mediumBytes)
        {
            var f = new FastImport(s);
            f.Commit("refs/heads/main", 0, 0, null, "Add a huge log file\n");
            var line = "2026-01-01T00:00:00Z INFO the quick brown fox jumps over the lazy dog, entry ";
            var text = new StringBuilder();
            for (long i = 0; text.Length < textBytes; i++) text.Append(line).Append(i).Append('\n');
            f.Modify("logs/huge.log", text.ToString());
            text.Clear();
            f.Commit("refs/heads/main", 1, 0, null, "Add a minified bundle on one line\n");
            for (long i = 0; text.Length < textBytes / 5; i++) text.Append("var a").Append(i).Append("=function(){return ").Append(i).Append("};");
            f.Modify("web/bundle.min.js", text.ToString());
            for (int r = 0; r < 3; r++)
            {
                f.Commit("refs/heads/main", 2 + r, 0, null, "Update the disk image, revision " + r + "\n");
                f.Raw("M 100644 inline assets/disk.img\n");
                f.RandomData(binaryBytes, 1000 + r);
            }
            f.Commit("refs/heads/main", 5, 0, null, "Add the textures\n");
            for (long i = 0; i < mediumCount; i++)
            {
                f.Raw("M 100644 inline textures/t" + i.ToString("0000") + ".png\n");
                f.RandomData(mediumBytes, 2000 + (int)i);
            }
        }

        // `.gitattributes` sending binaries through LFS, then `commits` commits each adding
        // `perCommit` LFS pointer files (whose objects aren't there) and changing two earlier ones.
        public static void LfsPointers(Stream s, long commits, long perCommit)
        {
            var f = new FastImport(s);
            f.Commit("refs/heads/main", 0, 0, null, "Track binaries with Git LFS\n");
            f.Modify(".gitattributes", "*.bin filter=lfs diff=lfs merge=lfs -text\n*.psd filter=lfs diff=lfs merge=lfs -text\n");
            long made = 0;
            using (var sha = SHA256.Create())
            {
                for (long c = 1; c <= commits; c++)
                {
                    f.Commit("refs/heads/main", c, 0, null);
                    for (long k = 0; k < perCommit; k++, made++) f.Modify(LfsPath(made), Pointer(sha, made + ":0"));
                    for (long k = 0; k < 2 && made > 0; k++)
                    {
                        long old = (c * 7 + k) % made;
                        f.Modify(LfsPath(old), Pointer(sha, old + ":" + c));
                    }
                }
            }
        }

        static string LfsPath(long i)
        {
            return "art/a" + (i / 100).ToString("000") + "/asset" + i + (i % 3 == 0 ? ".psd" : ".bin");
        }

        static string Pointer(SHA256 sha, string seed)
        {
            var hash = sha.ComputeHash(Encoding.UTF8.GetBytes(seed));
            var hex = new StringBuilder(64);
            foreach (var b in hash) hex.Append(b.ToString("x2"));
            long size = 1024 + (hash[0] << 16 | hash[1] << 8 | hash[2]);
            return "version https://git-lfs.github.com/spec/v1\noid sha256:" + hex + "\nsize " + size + "\n";
        }
    }
}
