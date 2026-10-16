#!/usr/bin/env python3
"""Static, comparable maps from actual Game fire arrivals; never opens a GUI.

MPLCONFIGDIR=/tmp/rocca-mpl .venv/bin/python tools/factory/scenario_images.py
Generate arrivals first with the headless `scenario_maps` Rust example.
"""
import json
import os
from pathlib import Path

os.environ.setdefault('MPLCONFIGDIR', '/tmp/rocca-mpl')
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import LightSource, to_rgb, ListedColormap, BoundaryNorm
from matplotlib.collections import LineCollection
from matplotlib.patches import Patch
import numpy as np

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT/'results/scenario-impacts/focused/images'
SOURCE = ROOT/'data/scenarios/rocca_ventosa'
BG = '#f7f5f0'
INK = '#23343c'
MUTED = '#62737a'
RED = '#b43835'
PURPLE = '#7b4b9d'
BLUE = '#20678b'
TITLES = {
    'impatti_valico_due_lati_30_6': ('01', 'Attraversamento del valico', '01_valico.png'),
    'impatti_nuclei_est_giro_lungo_28_6_75_60': ('02', 'Rotazione da est', '02_rotazione_est.png'),
    'impatti_nuclei_valico_costante_lungo_28_7_0_60': ('03', 'Pressione su quattro borghi', '03_quattro_borghi.png'),
}
LABELS = {
    'Fornaci': (3.4,6.4), 'Castelvento': (6.35,6.0), 'Le Terrazze': (3.1,5.05),
    'San Rocco': (6.9,3.6), 'Le Ghiande': (1.6,4.0), 'Pian dei Grilli': (1.7,2.15),
}
TOWN_ORDER = ['Fornaci','Castelvento','Le Terrazze','San Rocco','Le Ghiande','Pian dei Grilli']
FIRE_COLORS = ['#562a3e','#f8ce57','#fa913a','#e75135','#a82435']
FIRE_LABELS = ['Già acceso alla segnalazione','Entro +1 h','Tra +1 e +2 h','Tra +2 e +3 h','Oltre +3 h']
plt.rcParams.update({'font.family':'DejaVu Sans','font.size':12,'text.color':INK,'axes.labelcolor':INK})


def terrain():
    dem = np.fromfile(SOURCE/'dem.f64',dtype='<f8').reshape(400,400)
    fuel = np.fromfile(SOURCE/'fuel.i32',dtype='<i4').reshape(400,400)
    palette = ['#c7c7ac','#ccd09b','#bfc58c','#b1bf7d','#7e9d75','#718e65','#64845d',
               '#9ca77c','#919f6c','#83945c','#69836d','#5c7962','#527258']
    base = np.zeros((400,400,3))
    for code,color in enumerate(palette):base[fuel==code] = to_rgb(color)
    base[fuel<0] = to_rgb('#b6b8a7')
    shade = LightSource(azdeg=315,altdeg=42).hillshade(dem,vert_exag=1.5,dx=20,dy=20)
    rgb = np.clip(base*(.70+.40*shade[:,:,None]),0,1)
    rgb[dem<=0] = to_rgb('#d5e8ec')
    return rgb,dem


def draw(candidate,rgb,dem,roads):
    case = candidate['recipe']['case']
    name = candidate['name']
    number,title,filename = TITLES[name]
    result = json.loads((OUT/'data'/f'{name}.json').read_text())
    arrival = np.fromfile(OUT/'data'/f'{name}.i32',dtype='<i4').reshape(400,400)
    valid = arrival != np.iinfo(np.int32).min
    age = np.ma.array(arrival.astype(float)/60-case['start_min'],mask=~valid)
    # Exact grid cells, not an interpolated footprint or an ensemble forecast.
    fig = plt.figure(figsize=(15,10.3),facecolor=BG)
    fig.text(.045,.955,number,color=BLUE,fontsize=19,fontweight='bold')
    fig.text(.095,.954,title,fontsize=25,fontweight='bold')
    fig.text(.045,.913,'Stessa isola · seme 2 · simulazione SENZA ORDINI',fontsize=14,color=MUTED)
    ax = fig.add_axes([.035,.19,.625,.69])
    ax.imshow(rgb,extent=[0,8,0,8],origin='upper',zorder=0)
    ax.contour(np.linspace(.01,7.99,400),np.linspace(7.99,.01,400),dem,
               levels=[150,300,450],colors='#304c42',linewidths=.4,alpha=.13,zorder=1)
    lines = [np.array(r['line'])/1000 for r in roads if r.get('drivable')]
    ax.add_collection(LineCollection(lines,colors='#f4f2e9',linewidths=.85,alpha=.85,zorder=2))
    ax.imshow(age,extent=[0,8,0,8],origin='upper',cmap=ListedColormap(FIRE_COLORS),
              norm=BoundaryNorm([-31,0,60,120,180,1000],5),interpolation='nearest',alpha=.86,zorder=3)
    xy = np.array([h['pos'] for h in result['houses']])/1000
    ax.scatter(xy[:,0],xy[:,1],s=3,c='#344a42',alpha=.3,zorder=4,linewidths=0)
    for field,color,marker in [('caught',PURPLE,'o'),('alight',RED,'s')]:
        hit = np.array([h[field] for h in result['houses']])
        if hit.any():ax.scatter(xy[hit,0],xy[hit,1],s=11,c=color,marker=marker,zorder=5,edgecolors='white',linewidths=.25)
    boroughs = {d['name']:d for d in result['districts']}
    for town,d in boroughs.items():
        colour = RED if d['homes'] else PURPLE if d['caught'] else '#68796f'
        x,y = np.array(d['centre'])/1000
        ax.scatter([x],[y],s=66,c=colour,edgecolors='white',linewidths=1.3,zorder=7)
        ax.annotate(town,xy=(x,y),xytext=LABELS[town],fontsize=10.5,color=colour,ha='center',va='center',
                    fontweight='bold' if d['caught'] or d['homes'] else 'normal',
                    bbox=dict(boxstyle='round,pad=.3',facecolor=BG,edgecolor=colour,alpha=.97,linewidth=.65),
                    arrowprops=dict(arrowstyle='-',color=colour,lw=.8,alpha=.7),zorder=8)
    x,y = np.array(case['ignition'])/1000
    ax.scatter([x],[y],marker='*',s=230,c=INK,edgecolors='white',linewidths=1,zorder=10)
    ax.annotate('Innesco',xy=(x,y),xytext=(6.95,4.85),color=INK,fontsize=10,fontweight='bold',
                arrowprops=dict(arrowstyle='-',color=INK,lw=.9),
                bbox=dict(boxstyle='round,pad=.3',fc=BG,ec=INK,lw=.7),ha='center',zorder=10)
    ax.annotate('N',xy=(.7,7.35),xytext=(.7,6.75),ha='center',fontsize=12,fontweight='bold',
                arrowprops=dict(arrowstyle='-|>',color=INK,lw=1.5),zorder=10)
    ax.plot([.45,1.45],[.8,.8],color=INK,lw=2)
    ax.plot([.45,.45],[.74,.86],color=INK,lw=1.3)
    ax.plot([1.45,1.45],[.74,.86],color=INK,lw=1.3)
    ax.text(.95,.59,'1 km',ha='center',fontsize=10)
    ax.set(xlim=(0,8),ylim=(0,8),aspect='equal')
    ax.set_xticks([]);ax.set_yticks([])
    for spine in ax.spines.values():spine.set_visible(False)

    panel = fig.add_axes([.69,.19,.27,.69]);panel.set(xlim=(0,1),ylim=(0,1));panel.axis('off')
    panel.text(0,.98,'CONFIGURAZIONE',color=BLUE,fontsize=12,fontweight='bold')
    panel.text(0,.925,f'Innesco  ({case["ignition"][0]}, {case["ignition"][1]}) m',fontsize=13)
    panel.text(0,.875,f'Umidità combustibile  {case["moisture_pct"]}%',fontsize=13)
    hours,minutes = divmod(case['minutes']-case['start_min'],60)
    panel.text(0,.825,f'Durata dopo segnalazione  {hours} h {minutes:02d}',fontsize=13)
    panel.text(0,.755,f'Vento da {case["wind_from_deg"]}° · {case["wind_kmh"]} km/h',color=BLUE,fontsize=13,fontweight='bold')
    shift = case.get('shift')
    if shift:
        panel.text(0,.707,f'A +{shift["at_min"]-case["start_min"]} min: da {shift["wind_from_deg"]}° · {shift["wind_kmh"]} km/h',color=PURPLE,fontsize=12)
    else:panel.text(0,.707,'Direzione e intensità costanti',color=MUTED,fontsize=12)
    # Wind arrows show the direction of transport; the text gives the source.
    wind = fig.add_axes([.56,.76,.085,.105]);wind.set(xlim=(-1.2,1.2),ylim=(-1.2,1.2),aspect='equal');wind.axis('off')
    for direction,color,offset in [(case['wind_from_deg'],BLUE,-.35)]+([(shift['wind_from_deg'],PURPLE,.35)] if shift else []):
        angle=np.deg2rad(direction)
        wind.annotate('',xy=(offset-.75*np.sin(angle),-.75*np.cos(angle)),xytext=(offset,0),
                      arrowprops=dict(arrowstyle='-|>',lw=2,color=color,mutation_scale=17))
    wind.text(1.0,.95,'N',fontsize=9,color=MUTED,ha='center')
    panel.text(0,.575,'IMPATTI IN QUESTA SIMULAZIONE',color=BLUE,fontsize=11,fontweight='bold')
    panel.text(.65,.522,'Persone',ha='right',fontsize=10,color=MUTED)
    panel.text(1,.522,'Case',ha='right',fontsize=10,color=MUTED)
    for i,town in enumerate(TOWN_ORDER):
        d=boroughs[town];y=.465-i*.052
        colour=RED if d['homes'] else PURPLE if d['caught'] else MUTED
        panel.text(0,y,town,color=colour,fontsize=12)
        panel.text(.65,y,str(d['caught']),ha='right',fontsize=13,fontweight='bold' if d['caught'] else 'normal',color=colour)
        panel.text(1,y,str(d['homes']),ha='right',fontsize=13,fontweight='bold' if d['homes'] else 'normal',color=colour)
    panel.plot([0,1],[.175,.175],transform=panel.transAxes,color='#bdc7c2',lw=.8)
    panel.text(0,.125,'TOTALE',fontsize=11,fontweight='bold')
    panel.text(.65,.125,str(result['caught']),ha='right',fontsize=17,fontweight='bold')
    panel.text(1,.125,str(result['homes']),ha='right',fontsize=17,fontweight='bold')
    count=sum(bool(d['caught'] or d['homes']) for d in boroughs.values())
    panel.text(0,.055,f'{count} borghi colpiti · {result["ha"]:.0f} ha bruciati',fontsize=13,color=BLUE,fontweight='bold')
    counts=[len(e['all_boroughs']) for e in candidate['impact']['per_seed']]
    panel.text(0,0,f'Sui cinque semi: {min(counts)}–{max(counts)} borghi colpiti',fontsize=11,color=MUTED)

    fig.text(.045,.135,'QUANDO ARRIVA IL FUOCO',fontsize=10,fontweight='bold',color=MUTED)
    fig.legend([Patch(facecolor=c) for c in FIRE_COLORS],FIRE_LABELS,loc='lower left',
               bbox_to_anchor=(.038,.077),ncol=5,frameon=False,fontsize=10,handlelength=1.25,columnspacing=1.6)
    fig.text(.045,.054,'Tempi dalla segnalazione, avvenuta 30 min dopo l’innesco. Superfici e impatti sono simulati; nessun intervento.',fontsize=10,color=MUTED)
    fig.text(.045,.027,'Punti viola: persone colpite · quadrati rossi: case colpite · stella: innesco · territorio immaginario del gioco',fontsize=9,color=MUTED)
    fig.savefig(OUT/filename,dpi=150,facecolor=BG)
    plt.close(fig)
    print(OUT/filename)


def main():
    candidates=json.loads((OUT.parent/'variety-candidates.json').read_text())['candidates']
    rgb,dem=terrain()
    roads=json.loads((SOURCE/'osm.json').read_text())['roads']
    for candidate in sorted(candidates,key=lambda c:TITLES[c['name']][0]):draw(candidate,rgb,dem,roads)

if __name__=='__main__':main()
