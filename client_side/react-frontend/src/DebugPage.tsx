import { useEffect, useRef, useState } from "react";
import { HOST } from "./api/constants";
import { Box, FormControlLabel, IconButton, Paper, Popover, Switch, Typography } from "@mui/material";
import { Settings } from "@mui/icons-material";
import { cncAxios } from "./api/cncAxios";

export default function DebugPage() {
  const [displayedMessages, setDisplayedMessages] = useState<string[]>([]);
  const showStatus = useRef(false);
  const [showStatusRendered, setShowStatusRendered] = useState(showStatus.current);
  const messages = useRef<string[]>([]);
  const displayedIndices = useRef<string>("");
  // Note: will be set for sure before settings opens...
  const [settingsOpen, setSettingsOpen] = useState(false);
  const settingsButtonRef = useRef<HTMLButtonElement | null>(null);
  // The input...
  const inputHistory = useRef<string[]>([]);
  const inputHistoryPosition = useRef<number>(inputHistory.current.length);
  const [input, setInput] = useState("");
  const inputRef = useRef<HTMLInputElement | null>(null);

  function shouldShowMessage(message: string) {
    if(showStatus.current) {
      return true;
    } else {
      const lastCharacter = message[message.length - 1];
      return lastCharacter != "?" && lastCharacter != ">";
    }
  }
  function recomputeMessages() {
    const indicesToDisplay = [];
    for(let i = messages.current.length; i > 0; --i) {
      let index = i - 1;
      if(shouldShowMessage(messages.current[index])) {
        indicesToDisplay.push(index);
        if(indicesToDisplay.length >= 100) {
          break;
        }
      }
    }
    const key = JSON.stringify(indicesToDisplay);
    if(displayedIndices.current !== key) {
      displayedIndices.current = key;
      setDisplayedMessages(indicesToDisplay.map(index => messages.current[index]))
    }
  }
  useEffect(() => {
    const ws = new WebSocket(`ws://${HOST}/debug/listen_raw`);
    ws.onmessage = message => {
      messages.current.push(message.data as string);
      recomputeMessages();
    }
    return () => ws.close()
  }, []);
  useEffect(recomputeMessages, [showStatus]);
  return (
    <Paper sx={{height: "90vh", display: "flex", flexDirection: "column"}}>
      <Box
        overflow="scroll"
        display="flex"
        flexDirection="column-reverse"
        flexGrow={1}
        flexShrink={1}
        sx={{
          backgroundColor: "black",
          color: "white",
          fontWeight: "bold",
          paddingTop: "0.5em",
          paddingLeft: "2em",
          paddingRight: "2em",
          paddingBottom: "0.5em",
        }}
      >
        {
          // TODO: Key this...
          displayedMessages.map(message => (
            <div>{ message }</div>
          ))
        }
      </Box>
      <Box width="100%" display="flex">
        <IconButton ref={settingsButtonRef} onClick={ () => setSettingsOpen(!settingsOpen) }>
          <Settings/>
        </IconButton>
        <input
          ref={inputRef}
          type="text"
          style={{ border: "none", flexGrow: 1, outline: "none", fontSize: "large" }}
          value={input}
          onChange={(event) => setInput(event.target.value)}
          onKeyDown={(event) => {
            const element = inputRef.current!;
            if(event.key == "Enter") {
              inputHistory.current.push(input);
              inputHistoryPosition.current = inputHistory.current.length;
              cncAxios.post("/debug/send", input);
              setInput("");
            } else if(event.key == "ArrowUp") {
              const nextPosition = inputHistoryPosition.current - 1;
              if (nextPosition >= 0) {
                setInput(inputHistory.current[nextPosition]);
                inputHistoryPosition.current = nextPosition;
              }
              element.selectionStart = element.selectionEnd = element.value.length;
              event.preventDefault();
            } else if(event.key == "ArrowDown") {
              const nextPosition = inputHistoryPosition.current + 1;
              if (nextPosition == inputHistory.current.length) {
                inputHistoryPosition.current = nextPosition;
                setInput("");
              } else if(nextPosition < inputHistory.current.length) {
                inputHistoryPosition.current = nextPosition;
                setInput(inputHistory.current[nextPosition]);
              }
              element.selectionStart = element.selectionEnd = element.value.length;
              event.preventDefault();
            }
          }}
        />
        <Popover
          open={settingsOpen}
          anchorEl={settingsButtonRef.current}
          onClose={ () => setSettingsOpen(false) }
          transformOrigin={{ vertical: "bottom", horizontal: "left" }}
        >
          <Box display="flex">
          <Box padding={2}>
            <Typography variant="h6">Settings</Typography>
            <FormControlLabel
              control={<Switch onChange={ (_, checked) => {
                showStatus.current = checked;
                setShowStatusRendered(checked);
                recomputeMessages();
              }} checked={showStatusRendered} />}
              label="Show status queries"
            />
          </Box>
        </Box>
      </Popover>
      </Box>
    </Paper>
  )
}