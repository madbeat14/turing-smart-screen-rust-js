// Canvas module — manages the 480x320 editor canvas with drag/resize.
// Uses vanilla mouse events (no external dependencies).

/**
 * Initialize the canvas editor.
 * @param {HTMLElement} containerEl - The container for the canvas
 * @param {object} callbacks - { onSelect, onMove, onResize, onDrop }
 * @returns {object} Canvas controller
 */
function createCanvas(containerEl, callbacks) {
  var GRID_SIZE = 5;
  var HANDLE_SIZE = 8;
  var canvasEl = containerEl.querySelector('.canvas-area');
  var selectedId = null;
  var dragState = null; // { type: 'move'|'resize', widgetId, startX, startY, origX, origY, origW, origH, handle }
  var widgets = []; // Current widget list (reference from manifest)
  var scale = 1;

  function render(widgetList) {
    widgets = widgetList || widgets;
    canvasEl.innerHTML = '';

    // Calculate scale to fit container
    var containerRect = containerEl.getBoundingClientRect();
    var scaleX = (containerRect.width - 40) / 480;
    var scaleY = (containerRect.height - 40) / 320;
    scale = Math.min(scaleX, scaleY, 2);

    canvasEl.style.width = (480 * scale) + 'px';
    canvasEl.style.height = (320 * scale) + 'px';
    canvasEl.style.transform = '';

    for (var i = 0; i < widgets.length; i++) {
      var w = widgets[i];
      var el = document.createElement('div');
      el.className = 'canvas-widget' + (w.id === selectedId ? ' selected' : '');
      el.dataset.id = w.id;
      el.style.left = (w.x * scale) + 'px';
      el.style.top = (w.y * scale) + 'px';
      el.style.width = (w.width * scale) + 'px';
      el.style.height = (w.height * scale) + 'px';

      var typeInfo = WIDGET_TYPES[w.type];
      var label = typeInfo ? typeInfo.label : w.type;
      var configTitle = w.config && w.config.title ? w.config.title : (w.config && w.config.text ? w.config.text : '');

      el.innerHTML = '<div class="canvas-widget-label">' + escHtml(label) + '</div>' +
        (configTitle ? '<div class="canvas-widget-title">' + escHtml(configTitle) + '</div>' : '') +
        '<div class="canvas-widget-size">' + w.width + 'x' + w.height + '</div>';

      // Color-code by widget type
      var typeColors = {
        'metric-card': 'rgba(34,197,94,0.15)',
        'value-display': 'rgba(59,130,246,0.15)',
        'progress-bar': 'rgba(245,158,11,0.15)',
        'sparkline': 'rgba(168,85,247,0.15)',
        'clock': 'rgba(236,72,153,0.15)',
        'network-pair': 'rgba(59,130,246,0.15)',
        'label': 'rgba(255,255,255,0.08)',
        'divider': 'rgba(255,255,255,0.05)'
      };
      el.style.backgroundColor = typeColors[w.type] || 'rgba(255,255,255,0.08)';

      // Add resize handles if selected
      if (w.id === selectedId) {
        var handles = ['nw', 'ne', 'sw', 'se'];
        for (var h = 0; h < handles.length; h++) {
          var handle = document.createElement('div');
          handle.className = 'resize-handle resize-' + handles[h];
          handle.dataset.handle = handles[h];
          el.appendChild(handle);
        }
      }

      canvasEl.appendChild(el);
    }

    // Draw grid lines
    drawGrid();
  }

  function drawGrid() {
    // Grid is drawn via CSS background on the canvas element
    var gridPx = GRID_SIZE * scale;
    canvasEl.style.backgroundSize = gridPx + 'px ' + gridPx + 'px';
    canvasEl.style.backgroundImage = 'linear-gradient(to right, rgba(255,255,255,0.03) 1px, transparent 1px), linear-gradient(to bottom, rgba(255,255,255,0.03) 1px, transparent 1px)';
  }

  function snapToGrid(value) {
    return Math.round(value / GRID_SIZE) * GRID_SIZE;
  }

  function findWidgetAt(x, y) {
    // Find topmost widget at canvas coordinates
    for (var i = widgets.length - 1; i >= 0; i--) {
      var w = widgets[i];
      if (x >= w.x && x <= w.x + w.width && y >= w.y && y <= w.y + w.height) {
        return w;
      }
    }
    return null;
  }

  function canvasToWidget(clientX, clientY) {
    var rect = canvasEl.getBoundingClientRect();
    return {
      x: (clientX - rect.left) / scale,
      y: (clientY - rect.top) / scale
    };
  }

  // Mouse event handlers
  canvasEl.addEventListener('mousedown', function(e) {
    var pos = canvasToWidget(e.clientX, e.clientY);

    // Check for resize handle
    if (e.target.classList.contains('resize-handle') && selectedId) {
      var selWidget = widgets.find(function(w) { return w.id === selectedId; });
      if (selWidget) {
        dragState = {
          type: 'resize',
          widgetId: selectedId,
          handle: e.target.dataset.handle,
          startX: e.clientX,
          startY: e.clientY,
          origX: selWidget.x,
          origY: selWidget.y,
          origW: selWidget.width,
          origH: selWidget.height
        };
        e.preventDefault();
        return;
      }
    }

    // Check for widget click
    var clicked = findWidgetAt(pos.x, pos.y);
    if (clicked) {
      selectedId = clicked.id;
      dragState = {
        type: 'move',
        widgetId: clicked.id,
        startX: e.clientX,
        startY: e.clientY,
        origX: clicked.x,
        origY: clicked.y,
        origW: clicked.width,
        origH: clicked.height
      };
      render();
      if (callbacks.onSelect) callbacks.onSelect(clicked.id);
      e.preventDefault();
    } else {
      // Deselect
      if (selectedId) {
        selectedId = null;
        render();
        if (callbacks.onSelect) callbacks.onSelect(null);
      }
    }
  });

  document.addEventListener('mousemove', function(e) {
    if (!dragState) return;
    e.preventDefault();

    var dx = (e.clientX - dragState.startX) / scale;
    var dy = (e.clientY - dragState.startY) / scale;

    if (dragState.type === 'move') {
      var newX = snapToGrid(dragState.origX + dx);
      var newY = snapToGrid(dragState.origY + dy);
      // Clamp to canvas bounds
      newX = Math.max(0, Math.min(480 - dragState.origW, newX));
      newY = Math.max(0, Math.min(320 - dragState.origH, newY));

      if (callbacks.onMove) callbacks.onMove(dragState.widgetId, newX, newY);
    } else if (dragState.type === 'resize') {
      var handle = dragState.handle;
      var typeInfo = WIDGET_TYPES[widgets.find(function(w) { return w.id === dragState.widgetId; }).type];
      var minW = typeInfo ? typeInfo.minWidth : 20;
      var minH = typeInfo ? typeInfo.minHeight : 20;
      var newX2 = dragState.origX, newY2 = dragState.origY;
      var newW = dragState.origW, newH = dragState.origH;

      if (handle.indexOf('e') !== -1) {
        newW = snapToGrid(Math.max(minW, dragState.origW + dx));
      }
      if (handle.indexOf('w') !== -1) {
        var dxClamped = Math.min(dx, dragState.origW - minW);
        newX2 = snapToGrid(dragState.origX + dxClamped);
        newW = snapToGrid(dragState.origW - dxClamped);
      }
      if (handle.indexOf('s') !== -1) {
        newH = snapToGrid(Math.max(minH, dragState.origH + dy));
      }
      if (handle.indexOf('n') !== -1) {
        var dyClamped = Math.min(dy, dragState.origH - minH);
        newY2 = snapToGrid(dragState.origY + dyClamped);
        newH = snapToGrid(dragState.origH - dyClamped);
      }

      // Clamp to canvas
      if (newX2 < 0) { newW += newX2; newX2 = 0; }
      if (newY2 < 0) { newH += newY2; newY2 = 0; }
      if (newX2 + newW > 480) newW = 480 - newX2;
      if (newY2 + newH > 320) newH = 320 - newY2;

      if (callbacks.onResize) callbacks.onResize(dragState.widgetId, newX2, newY2, newW, newH);
    }
  });

  document.addEventListener('mouseup', function() {
    dragState = null;
  });

  // Keyboard: Delete selected widget
  document.addEventListener('keydown', function(e) {
    if (selectedId && (e.key === 'Delete' || e.key === 'Backspace')) {
      // Don't delete if user is typing in an input
      if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA' || e.target.tagName === 'SELECT') return;
      if (callbacks.onDelete) callbacks.onDelete(selectedId);
      selectedId = null;
    }
  });

  // Drop handler for widget palette
  canvasEl.addEventListener('dragover', function(e) {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'copy';
  });

  canvasEl.addEventListener('drop', function(e) {
    e.preventDefault();
    var widgetType = e.dataTransfer.getData('text/widget-type');
    if (widgetType && WIDGET_TYPES[widgetType]) {
      var pos = canvasToWidget(e.clientX, e.clientY);
      var snappedX = snapToGrid(pos.x);
      var snappedY = snapToGrid(pos.y);
      if (callbacks.onDrop) callbacks.onDrop(widgetType, snappedX, snappedY);
    }
  });

  return {
    render: render,
    getSelectedId: function() { return selectedId; },
    setSelected: function(id) { selectedId = id; render(); },
    getScale: function() { return scale; }
  };
}
