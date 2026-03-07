# git-disjoint demo
# Regenerate with: bash demo/generate.sh

Set Shell "bash"
Set FontSize 32
Set Width 1920
Set Height 1200
Set Framerate 30
Set PlaybackSpeed 0.75
Set Theme "Catppuccin Mocha"

Set TypingSpeed 40ms

Hide
Type "# First, make some commits on your default branch" Enter
Show

Type "./prepare-commits" Enter
Sleep 2s

Enter
Type "# Then, invoke git-disjoint!" Enter
Type "# It: " Enter
Type "# - creates a branch for each ticket," Enter
Type "# - cherry-picks related commits onto that branch," Enter
Type "# - then pushes each branch and opens a draft PR" Enter
Sleep 500ms

Set TypingSpeed 167ms
Type "git disjoint" Enter
Sleep 15s
