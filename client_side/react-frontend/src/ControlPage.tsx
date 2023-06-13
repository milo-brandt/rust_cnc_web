import { Box, Button, DialogActions, DialogContentText, DialogTitle, Divider, FormControl, FormControlLabel, FormLabel, IconButton, InputLabel, MenuItem, Paper, Radio, RadioGroup, Select, Tab, Table, TableBody, TableCell, TableContainer, TableHead, TableRow, Tabs, TextField, Tooltip, TooltipProps, Typography } from "@mui/material";
import { useStatusContext } from "./context/status";
import { PropsWithChildren, useState } from "react";
import { Add, ArrowRightAlt, ContentCopy, Delete, Edit, Error, Home, KeyboardArrowDown, KeyboardArrowLeft, KeyboardArrowRight, KeyboardArrowUp, KeyboardDoubleArrowDown, KeyboardDoubleArrowUp, LockOpen, RestartAlt, SettingsBackupRestore } from "@mui/icons-material";
import { feedOverride, home, rapidOverride, reset, spindleOverride, unlock } from "./api/immediateActions";
import { useSnackbar } from "./context/snackbar";
import { cncAxios } from "./api/cncAxios";
import { Maybe } from "./util/types";
import { useLocalStorage } from "./util/hooks";
import { OffsetKind, Offsets, SavedPosition, Vec3, deleteCoordinateOffset, recordPosition, setCoordinateOffset, useCoordinates, usePositions } from "./api/coords";
import { range, update } from "lodash";
import { DialogPromiseFactory, useDialog } from "./context/modal";
import { stringDialog } from "./dialog/string";

function CoordinateDisplay({ name, primary, secondary }: { name: string, primary: number, secondary: number}) {
  return (
    <>
      <Box fontSize="2em">{ name }</Box>
      <Box display="flex" flexDirection="column">
        <Box>{ primary.toFixed(2) }</Box>
        <Box color="gray" fontSize="0.75em">{ secondary.toFixed(2) }</Box>
      </Box>
    </>
  )
}

function BalancedDisplay({ children }: PropsWithChildren<{}>) {
  return <Box
    display="grid"
    gridTemplateColumns="50% 50%"
    alignItems="center"
    columnGap="1rem"
    rowGap="8px"
    sx={{
      '>:nth-child(2n + 1)': {
        textAlign: "right"
      }
    }}
  >
    { children }
  </Box>
}

function SimpleButton({handle, icon, title, errorText, tooltipPlacement = "bottom" }: {handle: () => Promise<any>, icon: JSX.Element, title: string, errorText: string, tooltipPlacement?: TooltipProps["placement"] }) {
  const { snackAsyncCatch } = useSnackbar();
  return (
    <Tooltip title={title} placement={tooltipPlacement}>
    <IconButton size="large" onClick={() => snackAsyncCatch(handle(), () => errorText) }>
      {icon}
    </IconButton>
  </Tooltip>

  )
}

function GlobalControls() {
  const { machineStatus } = useStatusContext();
  return (
    <>
    <Typography variant="h6">Setup</Typography>
    <SimpleButton title="Run Homing Cycle" icon={ <Home/> } handle={ home } errorText="Failed to home."/>
    <SimpleButton title="Unlock" icon={ <LockOpen/> } handle={ unlock } errorText="Failed to unlock."/>
    <SimpleButton title="Soft Reset" icon={ <RestartAlt/> } handle={ reset } errorText="Failed to reset."/>
    <Typography variant="h6">Feed Override ({ machineStatus.feed_override }%)</Typography>
    <SimpleButton title="Feed +10%" icon={ <KeyboardDoubleArrowUp/> } handle={ feedOverride.plus10 } errorText="Failed to override feed."/>
    <SimpleButton title="Feed +1%" icon={ <KeyboardArrowUp/> } handle={ feedOverride.plus1 } errorText="Failed to override feed."/>
    <SimpleButton title="Feed Reset" icon={ <SettingsBackupRestore/> } handle={ feedOverride.reset } errorText="Failed to override feed."/>
    <SimpleButton title="Feed -1%" icon={ <KeyboardArrowDown/> } handle={ feedOverride.minus1 } errorText="Failed to override feed."/>
    <SimpleButton title="Feed -10%" icon={ <KeyboardDoubleArrowDown/> } handle={ feedOverride.minus10 } errorText="Failed to override feed."/>
    <Typography variant="h6">Spindle Override ({ machineStatus.spindle_override }%)</Typography>
    <SimpleButton title="Spindle +10%" icon={ <KeyboardDoubleArrowUp/> } handle={ spindleOverride.plus10 } errorText="Failed to override spindle."/>
    <SimpleButton title="Spindle +1%" icon={ <KeyboardArrowUp/> } handle={ spindleOverride.plus1 } errorText="Failed to override spindle."/>
    <SimpleButton title="Spindle Reset" icon={ <SettingsBackupRestore/> } handle={ spindleOverride.reset } errorText="Failed to override spindle."/>
    <SimpleButton title="Spindle -1%" icon={ <KeyboardArrowDown/> } handle={ spindleOverride.minus1 } errorText="Failed to override spindle."/>
    <SimpleButton title="Spindle -10%" icon={ <KeyboardDoubleArrowDown/> } handle={ spindleOverride.minus10 } errorText="Failed to override spindle."/>
    <Typography variant="h6">Rapid Override ({ machineStatus.rapid_override }%)</Typography>
    <SimpleButton title="Rapid 100%" icon={ <div>1</div> } handle={ rapidOverride.reset } errorText="Failed to override rapids."/>
    <SimpleButton title="Rapid 50%" icon={ <div>&#189;</div> } handle={ rapidOverride.half } errorText="Failed to override rapids."/>
    <SimpleButton title="Rapid 25%" icon={ <div>&#188;</div> } handle={ rapidOverride.quarter } errorText="Failed to override rapids."/>
    </>
  )
}
function jog(x: number, y: number, z: number) {
  return cncAxios.post('/debug/send', `$J=G21 G91 F6000 X${ x.toFixed(3) } Y${ y.toFixed(3) } Z${ z.toFixed(3) }`)
}
function parseString(input: string): Maybe<number> {
  const parsed = input ? Number(input) : NaN;
  return isNaN(parsed) ? null : parsed;
}
function SmallNumberField ({ value, onChange, error }: { value: string, onChange: (value: string) => void, error: boolean }) {
  return <TextField
    variant="outlined"
    placeholder="Step"
    value={value}
    onChange={e => onChange(e.target.value)}
    error={error}
    sx={{
      '.MuiInputBase-input': {
        textAlign: 'center',
        padding: '8px',
      },
      width: "5rem",
    }}
  />

}

function JogController() {
  const [horizontalStep, setHorizontalStep] = useLocalStorage("horizontal-jog", () => "100");
  const horizontalStepSize = parseString(horizontalStep);

  const [verticalStep, setVerticalStep] = useLocalStorage("vertical-jog", () => "10");
  const verticalStepSize = parseString(verticalStep);

  const { snackAsyncCatch, createSnack} = useSnackbar();

  const jogCallback = (step: Maybe<number>, x: number, y: number, z: number) => async () => {
    if (step === null) {
      createSnack({
        message: 'Invalid step size.',
        severity: 'error'
      });
      return;
    }
    await snackAsyncCatch(jog(x * step, y * step, z * step), () => 'Failed to jog.');
  }

  return (
    <>
      <Typography variant="h6">Jog</Typography>
      <Box display="grid" gridTemplateColumns="max-content max-content max-content 4rem max-content" width="auto" alignItems="center" justifyItems="center">
        <Box gridColumn="1/4">X/Y</Box>
        <Box></Box>
        <Box>Z</Box>
        <Box></Box>
        <Box>
          <SimpleButton title="Y+" icon={ <KeyboardArrowUp/> } handle={ jogCallback(horizontalStepSize, 0, 1, 0) } errorText="Failed to jog." tooltipPlacement="top"/>
        </Box>
        <Box></Box>
        <Box></Box>
        <Box>
          <SimpleButton title="Z+" icon={ <KeyboardArrowUp/> } handle={ jogCallback(verticalStepSize, 0, 0, 1) } errorText="Failed to jog." tooltipPlacement="top"/>
        </Box>
    
        <Box>
          <SimpleButton title="X-" icon={ <KeyboardArrowLeft/> } handle={ jogCallback(horizontalStepSize, -1, 0, 0) } errorText="Failed to jog." tooltipPlacement="left"/>
        </Box>
        <Box>
          <SmallNumberField value={horizontalStep} onChange={setHorizontalStep} error={horizontalStepSize === null}/>
        </Box>
        <Box>
          <SimpleButton title="X+" icon={ <KeyboardArrowRight/> } handle={ jogCallback(horizontalStepSize, 1, 0, 0) } errorText="Failed to jog." tooltipPlacement="right"/>
        </Box>
        <Box></Box>
        <Box>
          <SmallNumberField value={verticalStep} onChange={setVerticalStep} error={verticalStepSize === null}/>
        </Box>

        <Box></Box>
        <Box>
          <SimpleButton title="Y-" icon={ <KeyboardArrowDown/> } handle={ jogCallback(horizontalStepSize, 0, -1, 0) } errorText="Failed to jog." tooltipPlacement="bottom"/>
        </Box>
        <Box></Box>
        <Box></Box>
        <Box>
          <SimpleButton title="Z-" icon={ <KeyboardArrowDown/> } handle={ jogCallback(verticalStepSize, 0, 0, -1) } errorText="Failed to jog." tooltipPlacement="bottom"/>
        </Box>
      </Box>
    </>
  )
}
function sub(a: Vec3, b: Vec3): Vec3 {
  return range(3).map(i => a[i] - b[i]) as Vec3
}
function add(a: Vec3, b: Vec3): Vec3 {
  return range(3).map(i => a[i] + b[i]) as Vec3
}
function formatCoordinates(v: Vec3) {
  return `X${ v[0].toFixed(2) } Y${ v[1].toFixed(2) } Z${ v[2].toFixed(2) }`
}
function PositionController({ positions, onAdd, currentPosition, wco, onSelect }: { positions: SavedPosition[], currentPosition: Vec3, onAdd: (label: string) => void, wco: Vec3, onSelect: (label: Maybe<string>, position: Vec3) => void }) {
  const [newLabel, setNewLabel] = useState("");
  const positionTable = (
    <TableContainer sx={{height: "12rem"}}>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>
              Coordinates*
            </TableCell>
            <TableCell>
              Label
            </TableCell>
            <TableCell>
              Action
            </TableCell>
          </TableRow>
          <TableRow>
            <TableCell>
              <Box fontWeight="initial" color="gray" display="inline" marginRight={4}>
                { formatCoordinates(sub(currentPosition, wco)) }
              </Box>
            </TableCell>
            <TableCell>
                <Box display="inline-flex">
                  <Box>
                    Add:
                  </Box>
                  <input
                    type="text"
                    style={{ border: "none", flexGrow: 1, outline: "none", marginLeft: "0.1rem", width: "7rem" }}
                    value={newLabel}
                    onChange={(event) => setNewLabel(event.target.value)}
                    onKeyDown={(event) => {
                      if(event.key === "Enter") {
                        onAdd(newLabel);
                        setNewLabel("");
                      }
                    }}
                  />
                </Box>
            </TableCell>
            <TableCell>
                  <IconButton size="small" onClick={ () => onSelect(null, currentPosition) }>
                    <KeyboardArrowRight/>
                  </IconButton>
                </TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {
            [...positions].reverse().map((value, index) => (
              <TableRow key={index}>
                <TableCell>
                  { formatCoordinates(sub(value.position, wco)) }
                </TableCell>
                <TableCell>
                  { value.label }
                </TableCell>
                <TableCell>
                  <IconButton size="small" onClick={ () => onSelect(value.label, value.position) }>
                    <KeyboardArrowRight/>
                  </IconButton>
                </TableCell>
              </TableRow>
            ))
          }
        </TableBody>
      </Table>
    </TableContainer>
  )
  return positionTable
}
function OffsetTable({ offsets, onCreate, onDelete }: { offsets: Record<string, Vec3>, onCreate: (copied: Maybe<string>) => void, onDelete: (value: string) => void }) {
  return <TableContainer>
    <Table size="small">
      <TableHead>
        <TableRow>
          <TableCell>Name</TableCell>
          <TableCell>Action</TableCell>
        </TableRow>
      </TableHead>
      <TableBody>
        <TableRow>
          <TableCell style={{fontStyle: "italic", color: "gray"}} >New</TableCell>
          <TableCell>
            <IconButton size="small" onClick={ () => onCreate(null) }>
              <Add/>
            </IconButton>
          </TableCell>
        </TableRow>
        {
          Object.entries(offsets).map(([name, _position]) => <TableRow>
            <TableCell key={ name }>
              { name }
            </TableCell>
            <TableCell>
              <IconButton size="small" onClick={ () => onCreate(name) }>
                <ContentCopy/>
              </IconButton>
              <IconButton size="small" onClick={ () => onDelete(name) }>
                <Delete/>
              </IconButton>
            </TableCell>
          </TableRow>)
        }
      </TableBody>
    </Table>
  </TableContainer>
}

function CoordinatePicker({ offsets, chosenTool, chosenWorkpiece, setTool, setWorkpiece }: { offsets: Offsets, chosenTool: string, chosenWorkpiece: string, setTool: (tool: string) => void, setWorkpiece: (workpiece: string) => void }) {
  return <Box>
    <FormControl fullWidth>
    <InputLabel id="tool-select-label">Tool</InputLabel>
      <Select
        labelId="tool-select-label"
        id="tool-select"
        label="Tool"
        value={chosenTool}
        onChange={(event) => setTool(event.target.value)}
      >
        <MenuItem key='' value='' style={{fontStyle: "italic", color: "gray"}}>Clear</MenuItem>
        {
          Object.keys(offsets.tools).map(key => (
            <MenuItem key={key} value={key}>{key} </MenuItem>
          ))
        }
      </Select>
      </FormControl>
      <FormControl fullWidth sx={{mt: "0.5rem"}}>
      <InputLabel id="workpiece-select-label">Workpiece</InputLabel>
      <Select
        labelId="workpiece-select-label"
        id="workpiece-select"
        label="Workpiece"
        value={chosenWorkpiece}
        onChange={(event) => setWorkpiece(event.target.value)}
      >
        <MenuItem key='' value='' style={{fontStyle: "italic", color: "gray"}}>Clear</MenuItem>
        {
          Object.keys(offsets.workpieces).map(key => (
            <MenuItem key={key} value={key}>{key} </MenuItem>
          ))
        }
      </Select>
    </FormControl>
  </Box>
}
function useCoordinateDialogFactory() {
  const { createDialogPromise } = useDialog();
  return (offsets: Offsets) => createDialogPromise<Maybe<{ tool: string, workpiece: string }>>(resolve => {
    let [chosenTool, setChosenTool] = useState<string>('');
    let [chosenWorkpiece, setChosenWorkpiece] = useState<string>('');
    return <>
      <DialogTitle>
        Select Coordinates
      </DialogTitle>
      <DialogContentText sx={{ml: "1rem", mr: "1rem", width: "20rem"}}>
        <CoordinatePicker
          offsets={offsets}
          chosenTool={chosenTool}
          setTool={setChosenTool}
          chosenWorkpiece={chosenWorkpiece}
          setWorkpiece={setChosenWorkpiece}
        />
      </DialogContentText>
      <DialogActions>
        <Button onClick={() => resolve(null)}>Cancel</Button>
        <Button onClick={() => resolve(chosenTool && chosenWorkpiece ? { tool: chosenTool, workpiece: chosenWorkpiece }: null)}>Set</Button>
      </DialogActions>
    </>
  })
}

function replaceIndex<T>(arr: T[], index: number, value: T): T[] {
  return [...arr.slice(0, index), value, ...arr.slice(index + 1)]
}
function parseOptionalNumber<T>(input: string): { type: "ok", value: Maybe<number> } | { type: "error" } {
  if(input === '') {
    return { type: "ok", value: null };
  }
  let number = Number(input);
  if (isNaN(number)) {
    return { type: "error" }
  } else {
    return { type: "ok", value: number };
  }
}

async function showCoordinateUpdateDialog(createDialogPromise: DialogPromiseFactory, { offsets, position }: {
  offsets: Offsets,
  position: Vec3
}): Promise<Maybe<{ tool: string, workpiece: string, target: OffsetKind, choices: Array<Maybe<number>> }>> {
  return createDialogPromise(resolve => {
    let [chosenTool, setChosenTool] = useState<string>('');
    let [chosenWorkpiece, setChosenWorkpiece] = useState<string>('');
    let [requestedPosition, setRequestedPosition] = useState<string[]>(['', '', '']);
    let [updateType, setUpdateType] = useState<OffsetKind>('Workpiece');
    let parsedPositions = requestedPosition.map(parseOptionalNumber);
    let parseOk = parsedPositions.every(value => value.type === 'ok');
    let positionChoices = parsedPositions.map(value => value.type === 'ok' ? value.value : null);
    let offset = chosenTool && chosenWorkpiece ? add(offsets.tools[chosenTool], offsets.workpieces[chosenWorkpiece]) : null;
    function createRow(label: string, index: number) {
      const parsed = parsedPositions[index];
      return <TableRow>
        <TableCell>
          { label }
          { offset ? (position[index] - offset[index]).toFixed(2) : '???' }
        </TableCell>
        <TableCell>
          { parsed.type === 'ok' ? parsed.value === null ? <></> : <ArrowRightAlt fontSize="small"/> : <Error fontSize="small"/> }
        </TableCell>
        <TableCell>
          <Box display="inline-flex" alignItems="center">
            <Box>{ label }</Box>
            <input
              type="text"
              style={{ border: "none", flexGrow: 1, outline: "none", marginLeft: "0.1rem", width: "7rem" }}
              value={requestedPosition[index]}
              onChange={(event) => setRequestedPosition(replaceIndex(requestedPosition, index, event.target.value)) }
            />
          </Box>
        </TableCell>
      </TableRow>
    }
    return <>
      <DialogTitle>
        Select Coordinates
      </DialogTitle>
      <DialogContentText sx={{ml: "1rem", mr: "1rem", width: "20rem"}}>
        <FormControl>
          <FormLabel id="demo-radio-buttons-group-label">Offset to Update</FormLabel>
          <RadioGroup
            row
            aria-labelledby="demo-radio-buttons-group-label"
            value={updateType}
            onChange={e => setUpdateType(e.target.value as OffsetKind)}
            name="radio-buttons-group"
          >
            <FormControlLabel value="Workpiece" control={<Radio />} label="Workpiece" />
            <FormControlLabel value="Tool" control={<Radio />} label="Tool" />
          </RadioGroup>
        </FormControl>
        <CoordinatePicker
          offsets={offsets}
          chosenTool={chosenTool}
          setTool={setChosenTool}
          chosenWorkpiece={chosenWorkpiece}
          setWorkpiece={setChosenWorkpiece}
        />
        <TableContainer>
          <Table>
            <TableBody>
              { createRow('X', 0) }
              { createRow('Y', 1) }
              { createRow('Z', 2) }
            </TableBody>
          </Table>
        </TableContainer>
      </DialogContentText>
      <DialogActions>
        <Button onClick={() => resolve(null)}>Cancel</Button>
        <Button
          disabled={!chosenTool || !chosenWorkpiece || !parseOk || positionChoices.every(value => value === null)}
          onClick={
            () => resolve(chosenTool && chosenWorkpiece ? { tool: chosenTool, workpiece: chosenWorkpiece, target: updateType as OffsetKind, choices: positionChoices }: null)
          }>Update</Button>
      </DialogActions>

    </>
  });
}

function CoordinateTab() {
  const { machineStatus } = useStatusContext();
  const { result: positionsResult, reload: reloadPositions } = usePositions();
  const { result: offsetsResult, reload: reloadOffsets } = useCoordinates(); 
  const [activeCoordinates, setActiveCoordinates] = useState<Maybe<{tool: string, workpiece: string}>>(null);
  const coordinateDialog = useCoordinateDialogFactory();
  const { createDialogPromise } = useDialog();
  const { snackAsyncCatch } = useSnackbar();

  async function storePosition(label: string) {
    await recordPosition({
      label,
      position: machineStatus.machine_position as Vec3
    });
    await reloadPositions();
  }
  if(positionsResult.status !== "resolved" || offsetsResult.status !== "resolved") {
    return (
      <div>"LOADING"</div>
    )
  }
  const wco = activeCoordinates ? add(
    offsetsResult.data.tools[activeCoordinates.tool],
    offsetsResult.data.workpieces[activeCoordinates.workpiece],
  ): machineStatus.work_coordinate_offset as Vec3

  async function createOffset(kind: OffsetKind, copiedFrom: Maybe<string>) {
    if(offsetsResult.status !== "resolved") {
      console.error("Unreachable");
      return;
    }
    const offsetSource = kind === "Tool" ? offsetsResult.data.tools : offsetsResult.data.workpieces;
    const offset = (copiedFrom ? offsetSource[copiedFrom] : null) ?? [0, 0, 0];
    const name = await stringDialog(createDialogPromise, {
      title: "Create offset",
      placeholder: "Offset label",
      action: "Create"
    });
    if (name !== null) {
      await snackAsyncCatch(setCoordinateOffset({ name, offset_kind: kind, offset }), () => `Failed to create offset ${name}`);
      reloadOffsets();
    }
  }
  async function deleteOffset(kind: OffsetKind, name: string) {
    await snackAsyncCatch(deleteCoordinateOffset({ offset_kind: kind, name }), () => `Failed to delete offset ${name}`);
    reloadOffsets();
  }
  async function updateOffset(position: Vec3) {
    if(offsetsResult.status !== "resolved") {
      return;
    }
    const result = await showCoordinateUpdateDialog(createDialogPromise, {
      offsets: offsetsResult.data,
      position
    });
    if(result !== null) {
      const unchangedComponent = result.target === 'Tool' ? offsetsResult.data.workpieces[result.workpiece] : offsetsResult.data.tools[result.tool];
      const changedComponent = result.target === 'Tool' ? offsetsResult.data.tools[result.tool] : offsetsResult.data.workpieces[result.workpiece];
      const newCoords = range(3).map(index => {
        const choice = result.choices[index];
        return choice === null ?
        changedComponent[index] // keep intact if not specified
        : position[index] - unchangedComponent[index] - choice // want position - (unchanged + changed) = choice 
      }) as Vec3;
      const targetName = result.target === 'Tool' ? result.tool : result.workpiece;
      await snackAsyncCatch(setCoordinateOffset({
        name: targetName,
        offset_kind: result.target,
        offset: newCoords,
      }), () => `Failed to update offset ${targetName}`);
      reloadOffsets();
    }
  }
  
  return (
    <>
      <JogController/>
      <Typography variant="h6">Saved Positions</Typography>
      <Box width="max-content">
        <PositionController positions={ positionsResult.data } onAdd={storePosition} wco={wco} currentPosition={machineStatus.machine_position as [number, number, number]} onSelect={ (_label, position) => updateOffset(position) }/>
        <Typography variant="h6">Offsets</Typography>
        <Box display="flex" gap={2}>
          <Box>
            Tools
            <OffsetTable offsets={ offsetsResult.data.tools } onCreate={(copiedFrom) => createOffset("Tool", copiedFrom) } onDelete={(name) => deleteOffset("Tool", name) }/>
          </Box>
          <Box>
            Workpieces
            <OffsetTable offsets={ offsetsResult.data.workpieces } onCreate={(copiedFrom) => createOffset("Workpiece", copiedFrom)} onDelete={(name) => deleteOffset("Workpiece", name)}/>
          </Box>
        </Box>
        <Box color="gray" fontSize="0.9em" mt="0.2rem">
          <Box fontStyle="italic" display="inline">*Relative to: </Box>
          { JSON.stringify(activeCoordinates) }
          { JSON.stringify(wco) }
          <IconButton size="small" onClick={ async () => {
            // TODO: Need to make sure setting state is async-safe...
            setActiveCoordinates(await coordinateDialog(offsetsResult.data))
           }}>
            <Edit/>
          </IconButton>
        </Box>
      </Box>
    </>
  )
}

export default function ControlPage() {
  const { machineStatus } = useStatusContext();
  const [activeTab, setActiveTab] = useState(1);
  /*
    Display for coordinates...
  */
  const coordinates = {X: 0, Y: 1, Z: 2};
  const coordinateDisplay = <BalancedDisplay>
    {
      Object.entries(coordinates).map(([name, index]) => {
        const absolute = machineStatus.machine_position[index];
        const relative = absolute - machineStatus.work_coordinate_offset[index];
        return <CoordinateDisplay key={name} name={name} primary={relative} secondary={absolute}/>
      })
    }
  </BalancedDisplay>
  const stateDisplay = <BalancedDisplay>
    <Box fontStyle="italic">Status</Box> <Box>{ machineStatus.state.type }</Box>
    <Box fontStyle="italic">Probe</Box> <Box><Tooltip title={ machineStatus.probe ? "Probe is reporting contact." : "Probe is not reporting contact." } placement="right"><Box sx={{ width: "max-content" }}>{ machineStatus.probe ? "On" : "Off" }</Box></Tooltip></Box>
  </BalancedDisplay>
  const overrideDisplay = <BalancedDisplay>
    <Box fontStyle="italic">Feed</Box> <Box>{ machineStatus.feed_override }%</Box>
    <Box fontStyle="italic">Rapid </Box> <Box>{ machineStatus.rapid_override }%</Box>
    <Box fontStyle="italic">Spindle</Box> <Box>{ machineStatus.spindle_override }%</Box>
  </BalancedDisplay>

  return (
    <Box display="flex" height="80vh">
      <Paper sx={{p: "1rem", height: "100%", whiteSpace: "nowrap"}} elevation={10} >
        <Typography variant="h4">Machine Status</Typography>
        <Typography variant="h6">Position</Typography>
         { coordinateDisplay }
         <Divider/>
         <Typography variant="h6">State</Typography>
         { stateDisplay }
         <Divider/>
         <Typography variant="h6">Overrides</Typography>
         { overrideDisplay }
      </Paper>
      <Box height="100%" width="100%" ml="1rem">
        <Box sx={{ borderBottom: 1, borderColor: 'divider', marginBottom: "0.5rem" }}>
          <Tabs value={activeTab} onChange={(_, value) => setActiveTab(value)}>
            <Tab label="Global" value={0}/>
            <Tab label="Coordinates" value={1}/>
          </Tabs>
        </Box>
        { activeTab == 0 ? <GlobalControls/> : null}
        { activeTab == 1 ? <CoordinateTab/> : null}
      </Box>
    </Box>
  )
}