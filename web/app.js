(() => {
  const OPCODE = {
    LIST_DATABASES: 0x01,
    CREATE_DATABASE: 0x02,
    DELETE_DATABASE: 0x03,
    DATABASE_STATUS: 0x04,
    PROVISION_GENESIS: 0x05,
    WRITE_MEMBRANE: 0x10,
    READ_MEMBRANE: 0x11,
    READ_MEMBRANE_FREE: 0x12,
    DELETE_MEMBRANE: 0x13,
    DERIVE_CELL: 0x20,
    FUSE_CELLS: 0x21,
    READ_AUTHENTICATED_CELL: 0x22,
  };

  const STATUS = {
    OK: 0x00,
    UNDEFINED: 0x01,
    UNAUTHORIZED: 0x02,
    ERROR: 0xff,
  };

  const GENE_NAMES = [
    "leer_self",
    "leer_any",
    "escribir_self",
    "escribir_any",
    "borrar_self",
    "borrar_any",
    "diferir",
    "fusionar",
    "clonar",
    "dominante",
    "leer_libre",
    "migrada",
    "custom1",
    "custom2",
    "custom3",
    "custom4",
    "custom5",
    "custom6",
    "custom7",
    "custom8",
    "custom9",
    "custom10",
    "custom11",
    "custom12",
    "custom13",
    "custom14",
    "custom15",
    "custom16",
    "custom17",
    "custom18",
    "custom19",
  ];

  const GENESIS_DEFAULT_MASK =
    (1 << 0) |
    (1 << 1) |
    (1 << 2) |
    (1 << 3) |
    (1 << 4) |
    (1 << 5) |
    (1 << 6) |
    (1 << 10);

  const state = {
    socket: null,
    databases: [],
    selectedDbId: null,
    selectedGenesis: null,
  };

  const ui = {
    wsDot: document.getElementById("ws-dot"),
    wsStatus: document.getElementById("ws-status"),
    selectedDb: document.getElementById("selected-db"),
    selectedGenesis: document.getElementById("selected-genesis"),
    dbMaxRecords: document.getElementById("db-max-records"),
    dbList: document.getElementById("db-list"),
    log: document.getElementById("log"),
    geneGrid: document.getElementById("gene-grid"),
    genomeMask: document.getElementById("genome-mask"),
    inspectOutput: document.getElementById("inspect-output"),
  };

  const textEncoder = new TextEncoder();
  const textDecoder = new TextDecoder();

  class Writer {
    constructor() {
      this.bytes = [];
    }

    u8(value) {
      this.bytes.push(value & 0xff);
    }

    u32(value) {
      const normalized = Number(value) >>> 0;
      this.bytes.push(normalized & 0xff, (normalized >>> 8) & 0xff, (normalized >>> 16) & 0xff, (normalized >>> 24) & 0xff);
    }

    uuid(uuid) {
      const hex = uuid.replace(/-/g, "");
      for (let index = 0; index < 16; index += 1) {
        this.bytes.push(parseInt(hex.slice(index * 2, index * 2 + 2), 16));
      }
    }

    bytesField(value) {
      this.u32(value.length);
      for (const byte of value) {
        this.bytes.push(byte);
      }
    }

    string(value) {
      this.bytesField(textEncoder.encode(value));
    }

    fixed16(value) {
      for (const byte of value) {
        this.bytes.push(byte);
      }
    }

    finish() {
      return new Uint8Array(this.bytes);
    }
  }

  class Reader {
    constructor(bytes) {
      this.view = bytes;
      this.offset = 0;
    }

    u8() {
      this.ensure(1);
      const value = this.view[this.offset];
      this.offset += 1;
      return value;
    }

    u32() {
      this.ensure(4);
      const base = this.offset;
      this.offset += 4;
      return (
        this.view[base] |
        (this.view[base + 1] << 8) |
        (this.view[base + 2] << 16) |
        (this.view[base + 3] << 24)
      ) >>> 0;
    }

    uuid() {
      this.ensure(16);
      const parts = [];
      for (let i = 0; i < 16; i += 1) {
        parts.push(this.view[this.offset + i].toString(16).padStart(2, "0"));
      }
      this.offset += 16;
      return `${parts.slice(0, 4).join("")}-${parts.slice(4, 6).join("")}-${parts.slice(6, 8).join("")}-${parts.slice(8, 10).join("")}-${parts.slice(10, 16).join("")}`;
    }

    bytesField() {
      const length = this.u32();
      this.ensure(length);
      const value = this.view.slice(this.offset, this.offset + length);
      this.offset += length;
      return value;
    }

    string() {
      return textDecoder.decode(this.bytesField());
    }

    optionalU32() {
      return this.u8() ? this.u32() : null;
    }

    optionalUuid() {
      return this.u8() ? this.uuid() : null;
    }

    fixed(length) {
      this.ensure(length);
      const value = this.view.slice(this.offset, this.offset + length);
      this.offset += length;
      return value;
    }

    ensure(length) {
      if (this.offset + length > this.view.length) {
        throw new Error("unexpected end of binary frame");
      }
    }
  }

  function log(message, payload) {
    const stamp = new Date().toLocaleTimeString();
    const body = payload === undefined ? `${stamp}  ${message}` : `${stamp}  ${message}\n${JSON.stringify(payload, null, 2)}`;
    ui.log.textContent = `${body}\n\n${ui.log.textContent}`.trim();
  }

  function setSocketStatus(connected, label) {
    ui.wsDot.classList.toggle("connected", connected);
    ui.wsStatus.textContent = label;
  }

  function renderGenes() {
    ui.geneGrid.innerHTML = "";
    GENE_NAMES.forEach((name, index) => {
      const wrapper = document.createElement("label");
      wrapper.className = "gene";
      wrapper.innerHTML = `<input type="checkbox" data-gene-bit="${index}"><span>${index}. ${name}</span>`;
      ui.geneGrid.appendChild(wrapper);
    });
    updateGenomeMask();
  }

  function updateGenomeMask() {
    let mask = 0;
    document.querySelectorAll("[data-gene-bit]").forEach((input) => {
      if (input.checked) {
        mask |= 1 << Number(input.dataset.geneBit);
      }
    });
    ui.genomeMask.textContent = String(mask >>> 0);
    return mask >>> 0;
  }

  function setGenomeMask(mask) {
    document.querySelectorAll("[data-gene-bit]").forEach((input) => {
      const bit = Number(input.dataset.geneBit);
      input.checked = Boolean(mask & (1 << bit));
    });
    updateGenomeMask();
  }

  function selectedDbIdRequired() {
    if (!state.selectedDbId) {
      throw new Error("select or create a database first");
    }
    return state.selectedDbId;
  }

  function selectedCellSpec() {
    return {
      secret: document.getElementById("cell-secret").value,
      salt: parseHex16(document.getElementById("cell-salt").value),
      genoma: updateGenomeMask(),
      x: numberValue("cell-x"),
      y: numberValue("cell-y"),
      z: numberValue("cell-z"),
    };
  }

  function childSpecFromForm() {
    const spec = selectedCellSpec();
    return {
      salt: spec.salt,
      genoma: spec.genoma,
      x: spec.x,
      y: spec.y,
      z: spec.z,
    };
  }

  function numberValue(id) {
    return Number(document.getElementById(id).value || 0) >>> 0;
  }

  function utf8Bytes(value) {
    return textEncoder.encode(value);
  }

  function parseHex16(value) {
    const cleaned = value.trim().replace(/[^0-9a-f]/gi, "").toLowerCase();
    if (cleaned.length !== 32) {
      throw new Error("salt must contain exactly 16 bytes encoded as 32 hex characters");
    }
    return hexToBytes(cleaned);
  }

  function hexToBytes(value) {
    const cleaned = value.trim().replace(/[^0-9a-f]/gi, "").toLowerCase();
    if (cleaned.length % 2 !== 0) {
      throw new Error("hex input must contain an even number of characters");
    }
    const bytes = new Uint8Array(cleaned.length / 2);
    for (let index = 0; index < cleaned.length; index += 2) {
      bytes[index / 2] = parseInt(cleaned.slice(index, index + 2), 16);
    }
    return bytes;
  }

  function bytesToHex(bytes) {
    return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  }

  function encodeChildSpec(writer, child) {
    writer.fixed16(child.salt);
    writer.u32(child.genoma);
    writer.u32(child.x);
    writer.u32(child.y);
    writer.u32(child.z);
  }

  function sendFrame(bytes) {
    if (!state.socket || state.socket.readyState !== WebSocket.OPEN) {
      throw new Error("websocket is not connected");
    }
    state.socket.send(bytes);
  }

  function requestListDatabases() {
    const writer = new Writer();
    writer.u8(OPCODE.LIST_DATABASES);
    sendFrame(writer.finish());
  }

  function requestCreateDatabase() {
    const writer = new Writer();
    writer.u8(OPCODE.CREATE_DATABASE);
    writer.u32(numberValue("db-max-records"));
    sendFrame(writer.finish());
  }

  function requestDeleteDatabase(dbId) {
    const writer = new Writer();
    writer.u8(OPCODE.DELETE_DATABASE);
    writer.uuid(dbId);
    sendFrame(writer.finish());
  }

  function requestDatabaseStatus() {
    const writer = new Writer();
    writer.u8(OPCODE.DATABASE_STATUS);
    writer.uuid(selectedDbIdRequired());
    sendFrame(writer.finish());
  }

  function requestProvisionGenesis() {
    const spec = selectedCellSpec();
    const writer = new Writer();
    writer.u8(OPCODE.PROVISION_GENESIS);
    writer.uuid(selectedDbIdRequired());
    writer.bytesField(utf8Bytes(spec.secret));
    writer.u32(spec.genoma);
    writer.u32(spec.x);
    writer.u32(spec.y);
    writer.u32(spec.z);
    sendFrame(writer.finish());
  }

  function requestWriteMembrane() {
    const writer = new Writer();
    writer.u8(OPCODE.WRITE_MEMBRANE);
    writer.uuid(selectedDbIdRequired());
    writer.string(document.getElementById("membrane-key").value);
    writer.bytesField(readMembraneValueBytes());
    writer.u32(numberValue("membrane-cell-index"));
    writer.bytesField(utf8Bytes(document.getElementById("membrane-secret").value));
    sendFrame(writer.finish());
  }

  function requestReadMembrane() {
    const writer = new Writer();
    writer.u8(OPCODE.READ_MEMBRANE);
    writer.uuid(selectedDbIdRequired());
    writer.string(document.getElementById("membrane-key").value);
    writer.u32(numberValue("membrane-cell-index"));
    writer.bytesField(utf8Bytes(document.getElementById("membrane-secret").value));
    sendFrame(writer.finish());
  }

  function requestReadMembraneFree() {
    const writer = new Writer();
    writer.u8(OPCODE.READ_MEMBRANE_FREE);
    writer.uuid(selectedDbIdRequired());
    writer.string(document.getElementById("membrane-key").value);
    sendFrame(writer.finish());
  }

  function requestDeleteMembrane() {
    const writer = new Writer();
    writer.u8(OPCODE.DELETE_MEMBRANE);
    writer.uuid(selectedDbIdRequired());
    writer.string(document.getElementById("membrane-key").value);
    writer.u32(numberValue("membrane-cell-index"));
    writer.bytesField(utf8Bytes(document.getElementById("membrane-secret").value));
    sendFrame(writer.finish());
  }

  function requestDeriveCell() {
    const writer = new Writer();
    writer.u8(OPCODE.DERIVE_CELL);
    writer.uuid(selectedDbIdRequired());
    writer.u32(numberValue("derive-parent-index"));
    writer.bytesField(utf8Bytes(document.getElementById("derive-parent-secret").value));
    writer.bytesField(utf8Bytes(document.getElementById("derive-child-secret").value));
    encodeChildSpec(writer, childSpecFromForm());
    sendFrame(writer.finish());
  }

  function requestFuseCells() {
    const writer = new Writer();
    writer.u8(OPCODE.FUSE_CELLS);
    writer.uuid(selectedDbIdRequired());
    writer.u32(numberValue("fuse-index-a"));
    writer.bytesField(utf8Bytes(document.getElementById("fuse-secret-a").value));
    writer.u32(numberValue("fuse-index-b"));
    writer.bytesField(utf8Bytes(document.getElementById("fuse-secret-b").value));
    writer.bytesField(utf8Bytes(document.getElementById("fuse-child-secret").value));
    encodeChildSpec(writer, childSpecFromForm());
    sendFrame(writer.finish());
  }

  function requestReadAuthenticatedCell() {
    const writer = new Writer();
    writer.u8(OPCODE.READ_AUTHENTICATED_CELL);
    writer.uuid(selectedDbIdRequired());
    writer.u32(numberValue("inspect-index"));
    writer.bytesField(utf8Bytes(document.getElementById("inspect-secret").value));
    sendFrame(writer.finish());
  }

  function readMembraneValueBytes() {
    const mode = document.getElementById("membrane-value-mode").value;
    const raw = document.getElementById("membrane-value").value;
    return mode === "hex" ? hexToBytes(raw) : utf8Bytes(raw);
  }

  function decodeResponse(arrayBuffer) {
    const reader = new Reader(new Uint8Array(arrayBuffer));
    const status = reader.u8();
    const opcode = reader.u8();

    if (status === STATUS.ERROR) {
      return { kind: "error", message: reader.string() };
    }

    if (status === STATUS.UNDEFINED) {
      return {
        kind: "undefined",
        dbId: reader.optionalUuid(),
        cellIndex: reader.optionalU32(),
      };
    }

    if (status === STATUS.UNAUTHORIZED) {
      return {
        kind: "unauthorized",
        dbId: reader.optionalUuid(),
        cellIndex: reader.optionalU32(),
        cellIndexA: reader.optionalU32(),
        cellIndexB: reader.optionalU32(),
      };
    }

    switch (opcode) {
      case OPCODE.LIST_DATABASES: {
        const count = reader.u32();
        const databases = [];
        for (let index = 0; index < count; index += 1) {
          databases.push({
            dbId: reader.uuid(),
            maxRecords: reader.u32(),
            genesisIndex: reader.optionalU32(),
          });
        }
        return { kind: "database-list", databases };
      }
      case OPCODE.CREATE_DATABASE:
      case OPCODE.DATABASE_STATUS:
        return {
          kind: opcode === OPCODE.CREATE_DATABASE ? "database-created" : "database-status",
          summary: {
            dbId: reader.uuid(),
            maxRecords: reader.u32(),
            genesisIndex: reader.optionalU32(),
          },
        };
      case OPCODE.DELETE_DATABASE:
        return { kind: "database-deleted", dbId: reader.uuid() };
      case OPCODE.PROVISION_GENESIS:
        return { kind: "genesis-provisioned", dbId: reader.uuid(), genesisIndex: reader.u32() };
      case OPCODE.WRITE_MEMBRANE:
        return { kind: "membrane-mutated", dbId: reader.uuid(), newCellIndex: reader.u32() };
      case OPCODE.READ_MEMBRANE:
        return {
          kind: "membrane-read",
          dbId: reader.uuid(),
          newCellIndex: reader.u32(),
          value: reader.bytesField(),
        };
      case OPCODE.READ_MEMBRANE_FREE:
        return { kind: "membrane-read-free", dbId: reader.uuid(), value: reader.bytesField() };
      case OPCODE.DERIVE_CELL:
        return {
          kind: "cell-derived",
          dbId: reader.uuid(),
          deferredIndex: reader.u32(),
          newCellIndex: reader.u32(),
        };
      case OPCODE.FUSE_CELLS:
        return {
          kind: "cells-fused",
          dbId: reader.uuid(),
          childIndex: reader.u32(),
          newCellIndexA: reader.u32(),
          newCellIndexB: reader.u32(),
        };
      case OPCODE.READ_AUTHENTICATED_CELL:
        return {
          kind: "authenticated-cell",
          dbId: reader.uuid(),
          cellIndex: reader.u32(),
          cell: reader.fixed(64),
        };
      default:
        throw new Error(`unsupported response opcode ${opcode}`);
    }
  }

  function renderDbList() {
    ui.dbList.innerHTML = "";
    if (state.databases.length === 0) {
      ui.dbList.innerHTML = `<div class="hint">No databases yet.</div>`;
      return;
    }

    for (const db of state.databases) {
      const item = document.createElement("div");
      item.className = `db-item${db.dbId === state.selectedDbId ? " active" : ""}`;
      item.innerHTML = `
        <strong>${db.genesisIndex === null ? "Uninitialized DB" : `Genesis ${db.genesisIndex}`}</strong>
        <div class="db-id">${db.dbId}</div>
        <div class="hint">max_records: ${db.maxRecords}</div>
        <div class="actions">
          <button data-select-db="${db.dbId}" class="secondary">Select</button>
          <button data-status-db="${db.dbId}" class="secondary">Status</button>
          <button data-delete-db="${db.dbId}" class="danger">Delete</button>
        </div>
      `;
      ui.dbList.appendChild(item);
    }

    ui.dbList.querySelectorAll("[data-select-db]").forEach((button) => {
      button.addEventListener("click", () => selectDatabase(button.dataset.selectDb));
    });
    ui.dbList.querySelectorAll("[data-status-db]").forEach((button) => {
      button.addEventListener("click", () => requestDatabaseStatusFor(button.dataset.statusDb));
    });
    ui.dbList.querySelectorAll("[data-delete-db]").forEach((button) => {
      button.addEventListener("click", () => requestDeleteDatabase(button.dataset.deleteDb));
    });
  }

  function selectDatabase(dbId) {
    state.selectedDbId = dbId;
    const summary = state.databases.find((entry) => entry.dbId === dbId);
    state.selectedGenesis = summary ? summary.genesisIndex : null;
    updateSelectedDbHeader();
    renderDbList();
  }

  function updateSelectedDbHeader() {
    ui.selectedDb.textContent = state.selectedDbId ?? "none";
    ui.selectedGenesis.textContent = state.selectedGenesis === null || state.selectedGenesis === undefined
      ? "none"
      : String(state.selectedGenesis);
  }

  function requestDatabaseStatusFor(dbId) {
    const writer = new Writer();
    writer.u8(OPCODE.DATABASE_STATUS);
    writer.uuid(dbId);
    sendFrame(writer.finish());
  }

  function mergeSummary(summary) {
    const index = state.databases.findIndex((entry) => entry.dbId === summary.dbId);
    if (index >= 0) {
      state.databases[index] = summary;
    } else {
      state.databases.push(summary);
    }
    if (!state.selectedDbId) {
      selectDatabase(summary.dbId);
    }
    if (state.selectedDbId === summary.dbId) {
      state.selectedGenesis = summary.genesisIndex;
      updateSelectedDbHeader();
    }
    renderDbList();
  }

  function handleResponse(response) {
    log(`response: ${response.kind}`, simplifyForLog(response));

    switch (response.kind) {
      case "database-list":
        state.databases = response.databases;
        if (state.selectedDbId && !state.databases.some((db) => db.dbId === state.selectedDbId)) {
          state.selectedDbId = null;
          state.selectedGenesis = null;
        }
        if (!state.selectedDbId && state.databases.length > 0) {
          selectDatabase(state.databases[0].dbId);
        } else {
          updateSelectedDbHeader();
          renderDbList();
        }
        break;
      case "database-created":
      case "database-status":
        mergeSummary(response.summary);
        break;
      case "database-deleted":
        state.databases = state.databases.filter((db) => db.dbId !== response.dbId);
        if (state.selectedDbId === response.dbId) {
          state.selectedDbId = null;
          state.selectedGenesis = null;
        }
        updateSelectedDbHeader();
        renderDbList();
        break;
      case "genesis-provisioned": {
        state.selectedGenesis = response.genesisIndex;
        const current = state.databases.find((db) => db.dbId === response.dbId);
        if (current) {
          current.genesisIndex = response.genesisIndex;
        }
        updateSelectedDbHeader();
        renderDbList();
        document.getElementById("membrane-cell-index").value = String(response.genesisIndex);
        document.getElementById("derive-parent-index").value = String(response.genesisIndex);
        document.getElementById("inspect-index").value = String(response.genesisIndex);
        document.getElementById("fuse-index-a").value = String(response.genesisIndex);
        document.getElementById("fuse-index-b").value = String(response.genesisIndex);
        break;
      }
      case "membrane-read":
      case "membrane-read-free": {
        const asText = safelyDecodeUtf8(response.value);
        const hex = bytesToHex(response.value);
        document.getElementById("membrane-value").value = asText ?? hex;
        document.getElementById("membrane-value-mode").value = asText === null ? "hex" : "text";
        if (response.newCellIndex !== undefined) {
          document.getElementById("membrane-cell-index").value = String(response.newCellIndex);
        }
        break;
      }
      case "membrane-mutated":
        document.getElementById("membrane-cell-index").value = String(response.newCellIndex);
        break;
      case "cell-derived":
        document.getElementById("derive-parent-index").value = String(response.newCellIndex);
        document.getElementById("membrane-cell-index").value = String(response.deferredIndex);
        document.getElementById("inspect-index").value = String(response.deferredIndex);
        break;
      case "cells-fused":
        document.getElementById("fuse-index-a").value = String(response.newCellIndexA);
        document.getElementById("fuse-index-b").value = String(response.newCellIndexB);
        document.getElementById("inspect-index").value = String(response.childIndex);
        break;
      case "authenticated-cell":
        ui.inspectOutput.innerHTML = `
          <div>db: ${response.dbId}</div>
          <div>cell index: ${response.cellIndex}</div>
          <div>raw hex: ${bytesToHex(response.cell)}</div>
        `;
        break;
      case "undefined":
      case "unauthorized":
      case "error":
        break;
      default:
        throw new Error(`unhandled response kind ${response.kind}`);
    }
  }

  function simplifyForLog(response) {
    const clone = { ...response };
    if (clone.value instanceof Uint8Array) {
      clone.valueHex = bytesToHex(clone.value);
      delete clone.value;
    }
    if (clone.cell instanceof Uint8Array) {
      clone.cellHex = bytesToHex(clone.cell);
      delete clone.cell;
    }
    return clone;
  }

  function safelyDecodeUtf8(bytes) {
    try {
      const text = textDecoder.decode(bytes);
      const normalized = utf8Bytes(text);
      if (normalized.length !== bytes.length) {
        return null;
      }
      for (let index = 0; index < bytes.length; index += 1) {
        if (normalized[index] !== bytes[index]) {
          return null;
        }
      }
      return text;
    } catch {
      return null;
    }
  }

  function guard(action) {
    try {
      action();
    } catch (error) {
      log("client error", { message: error.message });
    }
  }

  function attachEvents() {
    document.getElementById("db-create").addEventListener("click", () => guard(requestCreateDatabase));
    document.getElementById("db-refresh").addEventListener("click", () => guard(requestListDatabases));
    document.getElementById("db-status").addEventListener("click", () => guard(requestDatabaseStatus));
    document.getElementById("genesis-provision").addEventListener("click", () => guard(requestProvisionGenesis));

    document.getElementById("membrane-write").addEventListener("click", () => guard(requestWriteMembrane));
    document.getElementById("membrane-read").addEventListener("click", () => guard(requestReadMembrane));
    document.getElementById("membrane-read-free").addEventListener("click", () => guard(requestReadMembraneFree));
    document.getElementById("membrane-delete").addEventListener("click", () => guard(requestDeleteMembrane));

    document.getElementById("derive-run").addEventListener("click", () => guard(requestDeriveCell));
    document.getElementById("fuse-run").addEventListener("click", () => guard(requestFuseCells));
    document.getElementById("inspect-run").addEventListener("click", () => guard(requestReadAuthenticatedCell));

    document.getElementById("genes-genesis").addEventListener("click", () => setGenomeMask(GENESIS_DEFAULT_MASK));
    document.getElementById("genes-clear").addEventListener("click", () => setGenomeMask(0));
    document.addEventListener("change", (event) => {
      if (event.target instanceof HTMLElement && event.target.matches("[data-gene-bit]")) {
        updateGenomeMask();
      }
    });
  }

  function connect() {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const socket = new WebSocket(`${protocol}//${window.location.host}/ws`);
    socket.binaryType = "arraybuffer";
    state.socket = socket;

    socket.addEventListener("open", () => {
      setSocketStatus(true, "connected");
      log("websocket connected");
      requestListDatabases();
    });

    socket.addEventListener("close", () => {
      setSocketStatus(false, "disconnected; retrying...");
      log("websocket disconnected");
      window.setTimeout(connect, 1000);
    });

    socket.addEventListener("error", () => {
      setSocketStatus(false, "socket error");
    });

    socket.addEventListener("message", (event) => {
      try {
        handleResponse(decodeResponse(event.data));
      } catch (error) {
        log("decode error", { message: error.message });
      }
    });
  }

  renderGenes();
  setGenomeMask(GENESIS_DEFAULT_MASK);
  attachEvents();
  updateSelectedDbHeader();
  renderDbList();
  connect();
})();