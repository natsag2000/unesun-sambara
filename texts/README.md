# Greeting Text Files

This folder contains greeting text files that are randomly displayed when the editor starts.

## How to Add New Greeting Files

1. Create a new text file in this folder (e.g., `greeting_3.txt`, `greeting_4.txt`, or any name)
2. Write your greeting message in the file
3. Update the `manifest.json` file to include your new file

### Example:

If you create `greeting_3.txt`, update `manifest.json` like this:

```json
{
  "greetings": [
    "greeting_1.txt",
    "greeting_2.txt",
    "greeting_3.txt"
  ]
}
```

## File Format

- Plain text files (.txt)
- UTF-8 encoding
- Can include emoji and special characters (including Mongolian script)
- Newlines are preserved

## Current Files

- `greeting_1.txt` - Original welcome message
- `greeting_2.txt` - Alternative greeting
- `manifest.json` - List of all greeting files (must be updated when adding new files)
